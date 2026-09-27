import { forwardRef, lazy, Suspense, useEffect, useEffectEvent, useImperativeHandle, useMemo, useRef, useState } from "react";
import type { CSSProperties, Dispatch, MouseEvent, ReactNode, SetStateAction } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { buildDailyNoteMarkdown, getSemesterCourses, isoDate } from "../../lib/metrics";
import { createVault, deleteNote, importSummaryFiles, isTauriApp, linkVault, listNotes, listSummaryFiles, pickExistingVaultDirectory, pickSummaryFiles, pickVaultParentDirectory, readDailyNote, readNote, readReferenceNote, writeDailyNote, writeNote, writeReferenceNote } from "../../lib/obsidian";
import type { SummaryFile, VaultNoteFile } from "../../lib/obsidian";
import { makeId } from "../../lib/storage";
import type { AppState, Course } from "../../types";

const SummaryPdfViewer = lazy(() => import("../../components/SummaryPdfViewer").then((module) => ({ default: module.SummaryPdfViewer })));
type AppStyle = "modern" | "field-notebook" | "wabi-sabi";
type VaultSpace = "daily" | "references" | "summaries";
const vaultSpaces: Array<{ id: VaultSpace; label: string }> = [
  { id: "references", label: "References" },
  { id: "summaries", label: "Summaries" },
  { id: "daily", label: "Daily" },
];

const VAULT_FOLDER_PREVIEW: Array<{ name: string; hint: string }> = [
  { name: "Daily", hint: "one note per study day" },
  { name: "References", hint: "a page per course" },
  { name: "Summaries", hint: "imported PDFs and slides" },
];

function localIsoDate(date = new Date()) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function getErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return fallback;
}

function formatFileSize(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value >= 10 || unitIndex === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`;
}

function buildReferenceNoteMarkdown(course: Course) {
  return `# ${course.name} References

> Useful links, docs, recordings, exercises, and exam prep for ${course.name}.

## Official Links
- [Course page](https://example.com)
- [Moodle / LMS](https://example.com)

## Lecture Resources
- [Lecture recordings](https://example.com)
- [Slides folder](https://example.com)

## Exercises
- [Exercise sheets](https://example.com)
- [Solutions](https://example.com)

## Exam Prep
- [Past exams](https://example.com)
- [Formula sheet](https://example.com)
`;
}

function stripMarkdownFrontmatter(markdown: string) {
  return markdown.replace(/^---\n[\s\S]*?\n---\n+/, "");
}

function isExternalWebUrl(url: string) {
  return /^https?:\/\//i.test(url.trim());
}

function openExternalLink(url: string) {
  const target = url.trim();
  if (!isExternalWebUrl(target)) return;

  if (isTauriApp()) {
    void openUrl(target);
    return;
  }

  window.open(target, "_blank", "noreferrer");
}

function renderExternalLink(label: ReactNode, url: string, key: string) {
  return (
    <a
      key={key}
      href={url}
      onClick={(event: MouseEvent<HTMLAnchorElement>) => {
        event.preventDefault();
        openExternalLink(url);
      }}
      target="_blank"
      rel="noreferrer"
    >
      {label}
    </a>
  );
}

function renderInlineMarkdown(text: string, keyPrefix: string) {
  const parts = text.split(/(`[^`]+`|\*\*[^*]+\*\*|\*[^*]+\*|\[[^\]]+\]\([^)]+\)|https?:\/\/[^\s<)]+)/g).filter(Boolean);
  return parts.map((part, index) => {
    const key = `${keyPrefix}-${index}`;
    if (part.startsWith("`") && part.endsWith("`")) return <code key={key}>{part.slice(1, -1)}</code>;
    if (part.startsWith("**") && part.endsWith("**")) return <strong key={key}>{part.slice(2, -2)}</strong>;
    if (part.startsWith("*") && part.endsWith("*")) return <em key={key}>{part.slice(1, -1)}</em>;

    const link = part.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
    if (link) {
      return renderExternalLink(link[1], link[2], key);
    }

    if (isExternalWebUrl(part)) return renderExternalLink(part, part, key);

    return part;
  });
}

function renderMarkdownPreview(markdown: string) {
  const lines = markdown.split("\n");
  const elements: ReactNode[] = [];
  let listItems: string[] = [];
  let orderedItems: string[] = [];
  let codeLines: string[] = [];
  let inCode = false;

  function flushLists(key: string) {
    if (listItems.length) {
      elements.push(
        <ul key={`${key}-ul`}>
          {listItems.map((item, index) => <li key={index}>{renderInlineMarkdown(item, `${key}-ul-${index}`)}</li>)}
        </ul>,
      );
      listItems = [];
    }
    if (orderedItems.length) {
      elements.push(
        <ol key={`${key}-ol`}>
          {orderedItems.map((item, index) => <li key={index}>{renderInlineMarkdown(item, `${key}-ol-${index}`)}</li>)}
        </ol>,
      );
      orderedItems = [];
    }
  }

  lines.forEach((line, index) => {
    const key = `md-${index}`;
    const trimmed = line.trim();

    if (trimmed.startsWith("```")) {
      if (inCode) {
        elements.push(<pre key={key}><code>{codeLines.join("\n")}</code></pre>);
        codeLines = [];
        inCode = false;
      } else {
        flushLists(key);
        inCode = true;
      }
      return;
    }

    if (inCode) {
      codeLines.push(line);
      return;
    }

    if (!trimmed) {
      flushLists(key);
      return;
    }

    const heading = trimmed.match(/^(#{1,3})\s+(.+)$/);
    if (heading) {
      flushLists(key);
      const content = renderInlineMarkdown(heading[2], key);
      if (heading[1].length === 1) elements.push(<h1 key={key}>{content}</h1>);
      if (heading[1].length === 2) elements.push(<h2 key={key}>{content}</h2>);
      if (heading[1].length === 3) elements.push(<h3 key={key}>{content}</h3>);
      return;
    }

    if (trimmed.startsWith(">")) {
      flushLists(key);
      elements.push(<blockquote key={key}>{renderInlineMarkdown(trimmed.replace(/^>\s?/, ""), key)}</blockquote>);
      return;
    }

    const bullet = trimmed.match(/^[-*]\s+(.+)$/);
    if (bullet) {
      orderedItems = [];
      listItems.push(bullet[1]);
      return;
    }

    const ordered = trimmed.match(/^\d+\.\s+(.+)$/);
    if (ordered) {
      listItems = [];
      orderedItems.push(ordered[1]);
      return;
    }

    flushLists(key);
    elements.push(<p key={key}>{renderInlineMarkdown(trimmed, key)}</p>);
  });

  flushLists("end");
  if (inCode && codeLines.length) elements.push(<pre key="end-code"><code>{codeLines.join("\n")}</code></pre>);

  return elements;
}

export type VaultScreenHandle = { flush: () => Promise<boolean> };
export type { VaultSpace };
type Props = {
  state: AppState;
  setState: Dispatch<SetStateAction<AppState>>;
  appStyle: AppStyle;
  calendarToday: string;
  setMessage: (message: string) => void;
  /** Wabi-Sabi: the space (References / Summaries / Notes) is chosen from the sidebar instead of an in-page nav. */
  controlledSpace?: VaultSpace;
  onSpaceChange?: (space: VaultSpace) => void;
};

export const VaultScreen = forwardRef<VaultScreenHandle, Props>(function VaultScreen({ state, setState, appStyle, calendarToday, setMessage, controlledSpace, onSpaceChange }, ref) {
  const [vaultNoteDate, setVaultNoteDate] = useState(localIsoDate);
  const [vaultNoteContent, setVaultNoteContent] = useState("");
  const [vaultNotePath, setVaultNotePath] = useState<string | null>(null);
  const [vaultNotes, setVaultNotes] = useState<VaultNoteFile[]>([]);
  const [vaultNoteTitle, setVaultNoteTitle] = useState("");
  const [vaultNoteSavedAt, setVaultNoteSavedAt] = useState<number | null>(null);
  const [vaultNotesLoading, setVaultNotesLoading] = useState(false);
  const [vaultNoteDirty, setVaultNoteDirty] = useState(false);
  const [vaultNoteLoading, setVaultNoteLoading] = useState(false);
  const [vaultSetupOpen, setVaultSetupOpen] = useState(() => !state.settings.vaultPath);
  const [vaultDailyEditing, setVaultDailyEditing] = useState(false);
  const [markdownCheatsheetOpen, setMarkdownCheatsheetOpen] = useState(false);
  const [vaultSpace, setVaultSpace] = useState<VaultSpace>(controlledSpace ?? "daily");
  useEffect(() => {
    if (controlledSpace) setVaultSpace(controlledSpace);
  }, [controlledSpace]);
  useEffect(() => {
    onSpaceChange?.(vaultSpace);
  }, [vaultSpace, onSpaceChange]);
  const [referenceSemesterId, setReferenceSemesterId] = useState("");
  const [referenceCourseId, setReferenceCourseId] = useState("");
  const [referenceContent, setReferenceContent] = useState("");
  const [referencePath, setReferencePath] = useState<string | null>(null);
  const [referenceDirty, setReferenceDirty] = useState(false);
  const [referenceLoading, setReferenceLoading] = useState(false);
  const [referenceEditing, setReferenceEditing] = useState(false);
  const [referencePathVisible, setReferencePathVisible] = useState(false);
  const referenceLoadRequestRef = useRef(0);
  const [summarySemesterId, setSummarySemesterId] = useState("");
  const [summaryCourseId, setSummaryCourseId] = useState("");
  const [summaryFiles, setSummaryFiles] = useState<SummaryFile[]>([]);
  const [selectedSummaryPath, setSelectedSummaryPath] = useState<string | null>(null);
  const [summaryLoading, setSummaryLoading] = useState(false);
  const summaryLoadRequestRef = useRef(0);
  const [expandedSemesterIds, setExpandedSemesterIds] = useState<string[]>(() => state.semesters.map((semester) => semester.id));
  const toggleSemester = (semesterId: string) => setExpandedSemesterIds((current) => current.includes(semesterId) ? current.filter((id) => id !== semesterId) : [...current, semesterId]);

  useEffect(() => {
    if (!markdownCheatsheetOpen) return undefined;
    const closeOnEscape = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") setMarkdownCheatsheetOpen(false);
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [markdownCheatsheetOpen]);

  const semesterLookup = useMemo(
    () => new Map(state.semesters.map((semester) => [semester.id, semester])),
    [state.semesters],
  );
  // Archived semesters' subjects are hidden from every Vault selector below - references and
  // summaries are only ever browsed/edited for currently-active semesters.
  const activeSemesters = useMemo(() => state.semesters.filter((semester) => !semester.archived), [state.semesters]);
  const courseLookup = useMemo(
    () => new Map(state.courses.map((course) => [course.id, course])),
    [state.courses],
  );
  const referenceCourses = useMemo(
    () => referenceSemesterId ? state.courses.filter((course) => course.semesterId === referenceSemesterId) : [],
    [referenceSemesterId, state.courses],
  );
  const summaryCourses = useMemo(
    () => summarySemesterId ? state.courses.filter((course) => course.semesterId === summarySemesterId) : [],
    [summarySemesterId, state.courses],
  );
  const selectedReferenceSemester = referenceSemesterId ? semesterLookup.get(referenceSemesterId) ?? null : null;
  const selectedReferenceCourse = referenceCourseId ? courseLookup.get(referenceCourseId) ?? null : null;
  const selectedSummarySemester = summarySemesterId ? semesterLookup.get(summarySemesterId) ?? null : null;
  const selectedSummaryCourse = summaryCourseId ? courseLookup.get(summaryCourseId) ?? null : null;
  const selectedSummaryFile = summaryFiles.find((file) => file.path === selectedSummaryPath) ?? summaryFiles[0] ?? null;
  const selectedSummaryUrl = selectedSummaryFile ? convertFileSrc(selectedSummaryFile.path) : null;
  const loadReferenceNoteEvent = useEffectEvent(loadReferenceNote);
  const loadSummaryFileListEvent = useEffectEvent(loadSummaryFileList);
  const loadVaultNotesEvent = useEffectEvent(loadVaultNotes);
  const openVaultNoteEvent = useEffectEvent(openVaultNote);

  useEffect(() => {
    const firstSemester = activeSemesters[0]?.id ?? "";
    if (!referenceSemesterId && firstSemester) {
      setReferenceSemesterId(firstSemester);
      return;
    }
    if (referenceSemesterId && !activeSemesters.some((semester) => semester.id === referenceSemesterId)) {
      setReferenceSemesterId(firstSemester);
      setReferenceCourseId("");
    }
  }, [referenceSemesterId, activeSemesters]);

  useEffect(() => {
    const firstCourse = referenceCourses[0]?.id ?? "";
    if (!referenceCourseId && firstCourse) {
      setReferenceCourseId(firstCourse);
      return;
    }
    if (referenceCourseId && !referenceCourses.some((course) => course.id === referenceCourseId)) {
      setReferenceCourseId(firstCourse);
    }
  }, [referenceCourseId, referenceCourses]);

  useEffect(() => {
    setReferenceContent("");
    setReferencePath(null);
    setReferenceDirty(false);
    setReferenceEditing(false);
    setReferencePathVisible(false);
  }, [referenceSemesterId, referenceCourseId]);

  useEffect(() => {
    if (vaultSpace !== "references" || !state.settings.vaultPath || !selectedReferenceSemester || !selectedReferenceCourse) return;
    void loadReferenceNoteEvent(state.settings.vaultPath, selectedReferenceSemester, selectedReferenceCourse, { silent: true });
  }, [selectedReferenceCourse, selectedReferenceSemester, state.settings.vaultPath, vaultSpace]);

  useEffect(() => {
    const firstSemester = activeSemesters[0]?.id ?? "";
    if (!summarySemesterId && firstSemester) {
      setSummarySemesterId(firstSemester);
      return;
    }
    if (summarySemesterId && !activeSemesters.some((semester) => semester.id === summarySemesterId)) {
      setSummarySemesterId(firstSemester);
      setSummaryCourseId("");
    }
  }, [activeSemesters, summarySemesterId]);

  useEffect(() => {
    const firstCourse = summaryCourses[0]?.id ?? "";
    if (!summaryCourseId && firstCourse) {
      setSummaryCourseId(firstCourse);
      return;
    }
    if (summaryCourseId && !summaryCourses.some((course) => course.id === summaryCourseId)) {
      setSummaryCourseId(firstCourse);
    }
  }, [summaryCourseId, summaryCourses]);

  useEffect(() => {
    setSummaryFiles([]);
    setSelectedSummaryPath(null);
  }, [summarySemesterId, summaryCourseId]);

  useEffect(() => {
    if (vaultSpace !== "summaries" || !state.settings.vaultPath || !selectedSummarySemester || !selectedSummaryCourse) return;
    void loadSummaryFileListEvent(state.settings.vaultPath, selectedSummarySemester, selectedSummaryCourse, { silent: true });
  }, [selectedSummaryCourse, selectedSummarySemester, state.settings.vaultPath, vaultSpace]);

  useEffect(() => {
    if (appStyle !== "wabi-sabi" || !state.settings.vaultPath) return;
    void loadVaultNotesEvent(state.settings.vaultPath);
  }, [appStyle, state.settings.vaultPath]);

  useEffect(() => {
    if (appStyle !== "wabi-sabi" || vaultNoteTitle || !vaultNotes.length) return;
    void openVaultNoteEvent(vaultNotes[0]);
  }, [appStyle, vaultNoteTitle, vaultNotes]);

  const notePreview = useMemo(() => buildDailyNoteMarkdown({ courses: state.courses, sessions: state.sessions } as AppState, vaultNoteDate), [state.courses, state.sessions, vaultNoteDate]);
  const dailyPreviewContent = stripMarkdownFrontmatter(vaultNoteContent || notePreview);
  const referencePreview = selectedReferenceSemester && selectedReferenceCourse
    ? stripMarkdownFrontmatter(referenceContent || buildReferenceNoteMarkdown(selectedReferenceCourse))
    : "";

  async function handleCreateVault() {
    if (!isTauriApp()) {
      setMessage("Vault creation works inside the desktop build.");
      return;
    }

    try {
      const parent = await pickVaultParentDirectory();
      if (!parent) return;
      const vaultPath = await createVault(parent, state.settings.vaultName || "StudyTrackerVault");
      setState((current) => ({
        ...current,
        activeTab: "vault",
        settings: { ...current.settings, vaultPath },
      }));
      await loadVaultNote(vaultPath, vaultNoteDate);
      setMessage(`Vault created at ${vaultPath}`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not create the vault."));
    }
  }

  async function handleLinkVault() {
    if (!isTauriApp()) {
      setMessage("Linking a vault works inside the desktop build.");
      return;
    }

    try {
      const selected = await pickExistingVaultDirectory();
      if (!selected) return;
      const vaultPath = await linkVault(selected);
      setState((current) => ({
        ...current,
        activeTab: "vault",
        settings: { ...current.settings, vaultPath },
      }));
      await loadVaultNote(vaultPath, vaultNoteDate);
      setMessage(`Vault linked at ${vaultPath}`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not link the vault."));
    }
  }

  async function loadVaultNotes(vaultPath = state.settings.vaultPath) {
    if (!vaultPath) return;

    setVaultNotesLoading(true);
    try {
      setVaultNotes(await listNotes(vaultPath));
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not load notes."));
    } finally {
      setVaultNotesLoading(false);
    }
  }

  async function openVaultNote(note: VaultNoteFile) {
    if (!state.settings.vaultPath) return;

    setVaultNotesLoading(true);
    try {
      const content = await readNote(state.settings.vaultPath, note.title);
      setVaultNoteTitle(note.title);
      setVaultNoteContent(stripMarkdownFrontmatter(content ?? ""));
      setVaultNotePath(note.path);
      setVaultNoteSavedAt(note.savedAt);
      setVaultNoteDirty(false);
      setVaultDailyEditing(false);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not open note."));
    } finally {
      setVaultNotesLoading(false);
    }
  }

  function startNewVaultNote() {
    setVaultNoteTitle("Untitled note");
    setVaultNoteContent("");
    setVaultNotePath(null);
    setVaultNoteSavedAt(null);
    setVaultNoteDirty(false);
    setVaultDailyEditing(true);
  }

  async function handleSaveWabiVaultNote() {
    if (!state.settings.vaultPath) return;
    const title = vaultNoteTitle.trim();
    if (!title) {
      setMessage("Give the note a title before saving.");
      return;
    }

    setVaultNotesLoading(true);
    try {
      const notePath = await writeNote(state.settings.vaultPath, title, vaultNoteContent);
      const savedAt = Date.now();
      setVaultNotePath(notePath);
      setVaultNoteSavedAt(savedAt);
      setVaultNoteDirty(false);
      setVaultDailyEditing(false);
      await loadVaultNotes(state.settings.vaultPath);
      setMessage(`Saved ${title}.`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not save note."));
    } finally {
      setVaultNotesLoading(false);
    }
  }

  async function handleDeleteWabiVaultNote() {
    if (!state.settings.vaultPath || !vaultNotePath || !vaultNoteTitle) return;
    if (!window.confirm(`Delete "${vaultNoteTitle}"? This cannot be undone.`)) return;

    setVaultNotesLoading(true);
    try {
      await deleteNote(state.settings.vaultPath, vaultNoteTitle);
      const remaining = (await listNotes(state.settings.vaultPath)).filter((note) => note.title !== vaultNoteTitle);
      setVaultNotes(remaining);
      const nextNote = remaining[0];
      if (nextNote) {
        await openVaultNote(nextNote);
      } else {
        startNewVaultNote();
        setVaultDailyEditing(false);
      }
      setMessage(`Deleted ${vaultNoteTitle}.`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not delete note."));
    } finally {
      setVaultNotesLoading(false);
    }
  }

  async function loadVaultNote(vaultPath = state.settings.vaultPath, noteDate = vaultNoteDate) {
    if (!vaultPath) {
      setMessage("Create or link a vault first.");
      return;
    }

    setVaultNoteLoading(true);
    try {
      const existing = await readDailyNote(vaultPath, noteDate);
      setVaultNoteContent(existing ? stripMarkdownFrontmatter(existing) : buildDailyNoteMarkdown(state, noteDate));
      setVaultNotePath(`${vaultPath}/Daily/${noteDate}.md`);
      setVaultNoteDirty(false);
      setVaultDailyEditing(false);
      setMessage(existing ? `Loaded ${noteDate}.md` : `Created an unsaved draft for ${noteDate}.`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not load the daily note."));
    } finally {
      setVaultNoteLoading(false);
    }
  }

  async function handleSaveVaultNote() {
    if (!state.settings.vaultPath) {
      setMessage("Create or link a vault first.");
      return;
    }

    setVaultNoteLoading(true);
    try {
      const notePath = await writeDailyNote(state.settings.vaultPath, vaultNoteDate, vaultNoteContent || notePreview);
      setVaultNotePath(notePath);
      setVaultNoteDirty(false);
      setVaultDailyEditing(false);
      setState((current) => ({
        ...current,
        exports: [
          {
            id: makeId(),
            exportedAt: new Date().toISOString(),
            noteDate: vaultNoteDate,
            notePath,
          },
          ...current.exports,
        ].slice(0, 20),
      }));
      setMessage(`Saved daily note to ${notePath}`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not save the daily note."));
    } finally {
      setVaultNoteLoading(false);
    }
  }

  async function loadReferenceNote(
    vaultPath = state.settings.vaultPath,
    semester = selectedReferenceSemester,
    course = selectedReferenceCourse,
    options: { silent?: boolean } = {},
  ) {
    if (!vaultPath) {
      setMessage("Create or link a vault first.");
      return;
    }
    if (!semester || !course) {
      setMessage("Choose a semester and course first.");
      return;
    }

    const requestId = referenceLoadRequestRef.current + 1;
    referenceLoadRequestRef.current = requestId;
    setReferenceLoading(true);
    try {
      const existing = await readReferenceNote(vaultPath, semester.name, course.name);
      if (referenceLoadRequestRef.current !== requestId) return;
      setReferenceContent(existing ? stripMarkdownFrontmatter(existing) : buildReferenceNoteMarkdown(course));
      setReferencePath(`${vaultPath}/References/${semester.name}/${course.name}.md`);
      setReferenceDirty(false);
      setReferenceEditing(false);
      if (!options.silent) setMessage(existing ? `Loaded references for ${course.name}.` : `Created an unsaved references draft for ${course.name}.`);
    } catch (error) {
      if (referenceLoadRequestRef.current !== requestId) return;
      setMessage(getErrorMessage(error, "Could not load course references."));
    } finally {
      if (referenceLoadRequestRef.current === requestId) setReferenceLoading(false);
    }
  }

  async function handleSaveReferenceNote() {
    if (!state.settings.vaultPath) {
      setMessage("Create or link a vault first.");
      return;
    }
    if (!selectedReferenceSemester || !selectedReferenceCourse) {
      setMessage("Choose a semester and course first.");
      return;
    }

    setReferenceLoading(true);
    try {
      const content = referenceContent || buildReferenceNoteMarkdown(selectedReferenceCourse);
      const notePath = await writeReferenceNote(
        state.settings.vaultPath,
        selectedReferenceSemester.name,
        selectedReferenceCourse.name,
        content,
      );
      setReferenceContent(content);
      setReferencePath(notePath);
      setReferenceDirty(false);
      setReferenceEditing(false);
      setMessage(`Saved references to ${notePath}`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not save course references."));
    } finally {
      setReferenceLoading(false);
    }
  }

  function handleEditReferenceNote() {
    if (!selectedReferenceSemester || !selectedReferenceCourse) {
      setMessage("Choose a semester and course first.");
      return;
    }
    setReferenceContent((current) => current || buildReferenceNoteMarkdown(selectedReferenceCourse));
    setReferenceEditing(true);
  }

  async function loadSummaryFileList(
    vaultPath = state.settings.vaultPath,
    semester = selectedSummarySemester,
    course = selectedSummaryCourse,
    options: { silent?: boolean } = {},
  ) {
    if (!vaultPath) {
      setMessage("Create or link a vault first.");
      return;
    }
    if (!semester || !course) {
      setMessage("Choose a semester and course first.");
      return;
    }

    const requestId = summaryLoadRequestRef.current + 1;
    summaryLoadRequestRef.current = requestId;
    setSummaryLoading(true);
    try {
      const files = await listSummaryFiles(vaultPath, semester.name, course.name);
      if (summaryLoadRequestRef.current !== requestId) return;
      setSummaryFiles(files);
      setSelectedSummaryPath((current) => (current && files.some((file) => file.path === current) ? current : files[0]?.path ?? null));
      if (!options.silent) setMessage(files.length ? `Loaded ${files.length} summary file${files.length === 1 ? "" : "s"}.` : `No summaries found for ${course.name}.`);
    } catch (error) {
      if (summaryLoadRequestRef.current !== requestId) return;
      setMessage(getErrorMessage(error, "Could not load summaries."));
    } finally {
      if (summaryLoadRequestRef.current === requestId) setSummaryLoading(false);
    }
  }

  async function handleAddSummaryFiles() {
    if (!state.settings.vaultPath) {
      setMessage("Create or link a vault first.");
      return;
    }
    if (!selectedSummarySemester || !selectedSummaryCourse) {
      setMessage("Choose a semester and course first.");
      return;
    }
    if (!isTauriApp()) {
      setMessage("Adding summary files works inside the desktop build.");
      return;
    }

    const selected = await pickSummaryFiles();
    if (!selected.length) return;

    setSummaryLoading(true);
    try {
      const files = await importSummaryFiles(
        state.settings.vaultPath,
        selectedSummarySemester.name,
        selectedSummaryCourse.name,
        selected,
      );
      setSummaryFiles(files);
      const imported = new Set(selected.map((path) => path.split(/[\\/]/).pop()));
      const firstImported = files.find((file) => imported.has(file.name));
      setSelectedSummaryPath(firstImported?.path ?? files[0]?.path ?? null);
      setMessage(`Added ${selected.length} file${selected.length === 1 ? "" : "s"} to Summaries.`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not add summary files."));
    } finally {
      setSummaryLoading(false);
    }
  }

  function handleUseGeneratedNote() {
    setVaultNoteContent(notePreview);
    setVaultNoteDirty(true);
    setVaultDailyEditing(true);
    setMessage("Session draft copied into the editor. Save it when you are ready.");
  }

  const dirtyRef = useRef({ vaultNoteDirty, vaultNoteContent, vaultNoteDate, vaultNoteTitle, referenceDirty, referenceContent, selectedReferenceSemester, selectedReferenceCourse });
  dirtyRef.current = { vaultNoteDirty, vaultNoteContent, vaultNoteDate, vaultNoteTitle, referenceDirty, referenceContent, selectedReferenceSemester, selectedReferenceCourse };
  useImperativeHandle(ref, () => ({
    flush: async () => {
      const current = dirtyRef.current;
      const vaultPath = state.settings.vaultPath;
      if (!vaultPath) return true;
      try {
        if (current.vaultNoteDirty) {
          if (appStyle === "wabi-sabi") {
            const title = current.vaultNoteTitle.trim();
            if (!title) throw new Error("Give the note a title before leaving Vault.");
            await writeNote(vaultPath, title, current.vaultNoteContent);
          } else {
            await writeDailyNote(vaultPath, current.vaultNoteDate, current.vaultNoteContent || buildDailyNoteMarkdown(state, current.vaultNoteDate));
          }
          setVaultNoteDirty(false);
        }
        if (current.referenceDirty && current.selectedReferenceSemester && current.selectedReferenceCourse) {
          await writeReferenceNote(vaultPath, current.selectedReferenceSemester.name, current.selectedReferenceCourse.name, current.referenceContent || buildReferenceNoteMarkdown(current.selectedReferenceCourse));
          setReferenceDirty(false);
        }
        return true;
      } catch (error) {
        setMessage(getErrorMessage(error, "Could not save Vault changes."));
        return false;
      }
    },
  }), [appStyle, setMessage, state]);

  function renderWabiVaultCourseShelf(space: "references" | "summaries") {
    const selectedCourseId = space === "references" ? referenceCourseId : summaryCourseId;
    const showSemesterNames = activeSemesters.length > 1;
    return (
      <nav className="wabi-vault-tabs" aria-label={`${space} by course`}>
        {activeSemesters.map((semester) => (
          <div key={semester.id} className="wabi-vault-tab-group">
            {showSemesterNames ? <span className="wabi-vault-tab-semester">{semester.name}</span> : null}
            {getSemesterCourses(state, semester.id).map((course) => (
              <button
                key={course.id}
                type="button"
                className={`wabi-vault-tab ${course.id === selectedCourseId ? "active" : ""}`}
                title={course.name}
                onClick={() => {
                  if (space === "references") {
                    if (referenceDirty) {
                      setMessage("Save the current reference note before switching courses.");
                      return;
                    }
                    setReferenceSemesterId(semester.id);
                    setReferenceCourseId(course.id);
                    setReferenceEditing(false);
                  } else {
                    setSummarySemesterId(semester.id);
                    setSummaryCourseId(course.id);
                    setSelectedSummaryPath(null);
                  }
                }}
              >
                <span style={{ background: course.color }} />{course.name}
              </button>
            ))}
          </div>
        ))}
      </nav>
    );
  }
  return (
        <section className={`vault-shell ${appStyle === "field-notebook" ? "fn-vault" : appStyle === "wabi-sabi" ? "wabi-vault" : ""}`}>
          {appStyle === "field-notebook" ? (
            <aside className="fn-vault-rail" aria-label="Vault drawers">
              <div className="fn-vault-status">
                <span className={state.settings.vaultPath ? "linked" : ""} />
                <strong>{state.settings.vaultName || "StudyTrackerVault"}</strong>
                <em>{state.settings.vaultPath ? "SYNCED" : "NOT LINKED"}</em>
              </div>
              {state.settings.vaultPath ? (
                <div className="fn-vault-tabs">
                  {vaultSpaces.map((space) => (
                    <button key={space.id} type="button" className={vaultSpace === space.id ? "active" : ""} onClick={() => setVaultSpace(space.id)}>{space.label}</button>
                  ))}
                </div>
              ) : null}
              <div className="fn-vault-tools">
                <button type="button" onClick={() => setMarkdownCheatsheetOpen(true)}>Markdown</button>
                {state.settings.vaultPath ? (
                  <button type="button" onClick={() => setVaultSetupOpen((current) => !current)}>{vaultSetupOpen ? "Close setup" : "Vault setup"}</button>
                ) : null}
              </div>
              {!state.settings.vaultPath ? (
                <p className="fn-vault-rail-hint">Drawers appear once a vault folder is linked.</p>
              ) : vaultSpace === "daily" ? (
                <>
                  <div className="fn-rail-section">
                    <div className="fn-rail-label">Recent daily notes</div>
                    <button type="button" className="fn-rail-row active" onClick={() => setVaultNoteDate(calendarToday)}>
                      <strong>{calendarToday}</strong>
                      <span>{state.sessions.filter((session) => isoDate(new Date(session.endedAt)) === calendarToday).length} sessions · today</span>
                    </button>
                    {state.exports.slice(0, 5).map((item) => (
                      <button key={item.id} type="button" className="fn-rail-row" onClick={() => setVaultNoteDate(item.noteDate)}>
                        <strong>{item.noteDate}</strong>
                        <span>{item.notePath}</span>
                      </button>
                    ))}
                  </div>
                </>
              ) : (
                <div className="fn-rail-section fn-vault-tree">
                  <div className="fn-rail-label">Course drawers</div>
                  {activeSemesters.map((semester) => {
                    const courses = getSemesterCourses(state, semester.id);
                    const activeSemester = expandedSemesterIds.includes(semester.id);
                    return (
                      <div key={semester.id} className="fn-semester-tree">
                        <button type="button" className={`fn-rail-row fn-semester-folder ${activeSemester ? "active" : ""}`} onClick={() => toggleSemester(semester.id)}>
                          <strong>{semester.name}</strong>
                          <span>{activeSemester ? "close" : "open"} · {courses.length} courses</span>
                        </button>
                        {activeSemester ? (
                          <div className="fn-course-tree">
                            {courses.map((course) => {
                              const activeCourse = vaultSpace === "references" ? course.id === referenceCourseId : course.id === summaryCourseId;
                              return (
                                <div key={course.id} className="fn-course-tree-item">
                                  <button type="button" className={`fn-course-folder ${activeCourse ? "active" : ""}`} style={{ "--fn-course": course.color } as CSSProperties} onClick={() => {
                                    if (vaultSpace === "references") {
                                      if (referenceDirty) {
                                        setMessage("Save the current reference note before switching courses.");
                                        return;
                                      }
                                      setReferenceSemesterId(semester.id);
                                      setReferenceCourseId(course.id);
                                    } else {
                                      setSummarySemesterId(semester.id);
                                      setSummaryCourseId(course.id);
                                    }
                                  }}>
                                    <strong>{course.name}</strong>
                                    <span>{vaultSpace === "references" ? "note" : vaultSpace === "summaries" && activeCourse ? summaryFiles.length : "files"}</span>
                                  </button>
                                  {vaultSpace === "summaries" && activeCourse ? (
                                    <div className="fn-task-tree fn-summary-files-tree">
                                      <div className="fn-sidebar-actions fn-summary-file-actions">
                                        <button type="button" onClick={() => loadSummaryFileList()} disabled={summaryLoading}>{summaryLoading ? "loading" : "refresh"}</button>
                                        <button type="button" onClick={handleAddSummaryFiles} disabled={summaryLoading}>add files</button>
                                      </div>
                                      {summaryFiles.length ? summaryFiles.map((file) => (
                                        <button key={file.path} type="button" className={`fn-sidebar-task ${file.path === selectedSummaryFile?.path ? "active" : ""}`} onClick={() => setSelectedSummaryPath(file.path)}>
                                          <span>{file.name}</span>
                                          <em>{file.kind.toUpperCase()}</em>
                                        </button>
                                      )) : <p className="fn-sidebar-empty">No files yet.</p>}
                                    </div>
                                  ) : null}
                                </div>
                              );
                            })}
                          </div>
                        ) : null}
                      </div>
                    );
                  })}
                </div>
              )}
            </aside>
          ) : null}
          {appStyle !== "field-notebook" ? <div className="vault-hero">
            <div>
              <h1>Vault</h1>
              <p>Your plain-markdown knowledge base, stored as files you own.</p>
            </div>
            <div className="vault-hero-actions">
              {state.settings.vaultPath ? (
                <span className="vault-status-pill"><span />{state.settings.vaultName || "Linked vault"}</span>
              ) : null}
              <button type="button" className="ghost-button" data-tour="vault-markdown" onClick={() => setMarkdownCheatsheetOpen(true)}>
                Markdown
              </button>
              {state.settings.vaultPath ? (
                <button type="button" className="ghost-button vault-settings-button" data-tour="vault-setup" onClick={() => setVaultSetupOpen((current) => !current)}>
                  {vaultSetupOpen ? "Close setup" : "Vault setup"}
                </button>
              ) : null}
            </div>
          </div> : null}

          {markdownCheatsheetOpen ? (
            <div className="markdown-cheatsheet-backdrop" onMouseDown={() => setMarkdownCheatsheetOpen(false)}>
              <section className="markdown-cheatsheet-panel" onMouseDown={(event) => event.stopPropagation()} aria-label="Markdown cheatsheet">
                <div className="markdown-cheatsheet-head">
                  <div>
                    <p className="eyebrow">Vault markdown</p>
                    <h2>Cheatsheet</h2>
                    <p>These are the markdown features Study Tracker currently previews.</p>
                  </div>
                  <button type="button" className="ghost-button small-button" onClick={() => setMarkdownCheatsheetOpen(false)} aria-label="Close markdown cheatsheet">
                    X
                  </button>
                </div>

                <div className="markdown-cheatsheet-grid">
                  <div className="markdown-cheatsheet-example">
                    <h3>Headings</h3>
                    <pre><code>{"# Title\n## Section\n### Subsection"}</code></pre>
                  </div>
                  <div className="markdown-cheatsheet-example">
                    <h3>Emphasis</h3>
                    <pre><code>{"**bold**\n*italic*\n`inline code`"}</code></pre>
                  </div>
                  <div className="markdown-cheatsheet-example">
                    <h3>Lists</h3>
                    <pre><code>{"- bullet item\n* another bullet\n\n1. first\n2. second"}</code></pre>
                  </div>
                  <div className="markdown-cheatsheet-example">
                    <h3>Quotes</h3>
                    <pre><code>{"> Important note or reminder"}</code></pre>
                  </div>
                  <div className="markdown-cheatsheet-example">
                    <h3>Links</h3>
                    <pre><code>{"[ETH Video](https://video.ethz.ch/...)\nhttps://video.ethz.ch/..."}</code></pre>
                  </div>
                  <div className="markdown-cheatsheet-example">
                    <h3>Code Blocks</h3>
                    <pre><code>{"```\nconst topic = \"series\";\n```"}</code></pre>
                  </div>
                </div>

                <div className="markdown-cheatsheet-note">
                  <strong>Not supported in preview yet:</strong> tables, images, LaTeX/math rendering, nested lists, and interactive task checkboxes.
                </div>
              </section>
            </div>
          ) : null}

          {!state.settings.vaultPath ? (
            <article className="panel-card vault-onboard-card">
              <div className="vault-onboard-head">
                <p className="eyebrow">No vault linked</p>
                <h2>Set up your notes vault</h2>
                <p className="section-note">A vault is just a folder on your computer. Study Tracker writes plain <code>.md</code> files into it, so every note stays readable in any editor and stays yours.</p>
              </div>

              <div className="vault-onboard-body">
                <div className="vault-onboard-preview" aria-hidden="true">
                  <span className="vault-onboard-root">{(state.settings.vaultName || "StudyTrackerVault")}/</span>
                  {VAULT_FOLDER_PREVIEW.map((folder) => (
                    <span key={folder.name} className="vault-onboard-branch">
                      <em>{folder.name}/</em>
                      <i>{folder.hint}</i>
                    </span>
                  ))}
                </div>

                <div className="vault-onboard-form">
                  <label className="field">
                    <span>Vault name</span>
                    <input
                      data-tour="vault-name"
                      value={state.settings.vaultName}
                      onChange={(event) => setState((current) => ({ ...current, settings: { ...current.settings, vaultName: event.target.value } }))}
                      placeholder="StudyTrackerVault"
                    />
                  </label>
                  <div className="vault-onboard-actions">
                    <button type="button" data-tour="vault-create" onClick={handleCreateVault}>Create new vault</button>
                    <button type="button" className="ghost-button" data-tour="vault-link" onClick={handleLinkVault}>Link existing folder</button>
                  </div>
                  <p className="vault-onboard-foot">Create builds a fresh folder with the structure on the left. Link points Study Tracker at a folder you already keep notes in.</p>
                </div>
              </div>
            </article>
          ) : null}

          {state.settings.vaultPath && vaultSetupOpen ? (
            <article className="panel-card vault-settings-panel">
              <div className="section-head">
                <div>
                  <p className="eyebrow">Markdown vault</p>
                  <h2>Vault configuration</h2>
                </div>
              </div>
              <div className="form-grid compact-grid">
                <label className="field">
                  <span>Vault name</span>
                  <input
                    data-tour="vault-name"
                    value={state.settings.vaultName}
                    onChange={(event) => setState((current) => ({ ...current, settings: { ...current.settings, vaultName: event.target.value } }))}
                    placeholder="StudyTrackerVault"
                  />
                </label>
                <label className="field wide">
                  <span>Current vault path</span>
                  <input data-tour="vault-path" value={state.settings.vaultPath ?? ""} readOnly />
                </label>
              </div>
              <div className="vault-folder-strip" aria-label="Vault folders">
                {VAULT_FOLDER_PREVIEW.map((folder) => <span key={folder.name}>{folder.name}</span>)}
              </div>
              <div className="control-row left roomy-top">
                <button type="button" data-tour="vault-create" onClick={handleCreateVault}>Create new vault</button>
                <button type="button" className="ghost-button" data-tour="vault-link" onClick={handleLinkVault}>Link existing folder</button>
              </div>
            </article>
          ) : null}

          {state.settings.vaultPath ? (
            <>
              {appStyle !== "wabi-sabi" ? (
                <nav className="vault-nav" aria-label="Vault spaces" data-tour="vault-spaces">
                  {vaultSpaces.map((space) => {
                    const active = space.id === vaultSpace;
                    return (
                      <button
                        key={space.id}
                        type="button"
                        className={`vault-nav-item ${active ? "active" : ""}`}
                        onClick={() => setVaultSpace(space.id)}
                      >
                        {space.label}
                      </button>
                    );
                  })}
                </nav>
              ) : null}

              {appStyle === "wabi-sabi" && !controlledSpace ? (
                <nav className="wabi-notes-nav wabi-vault-space-nav" aria-label="Vault spaces" data-tour="vault-spaces">
                  {vaultSpaces.map((space) => (
                    <button key={space.id} type="button" className={vaultSpace === space.id ? "active" : ""} onClick={() => setVaultSpace(space.id)}>
                      {space.id === "daily" ? "Notes" : space.label}
                    </button>
                  ))}
                </nav>
              ) : null}

              {vaultSpace === "daily" ? (
                appStyle === "wabi-sabi" ? (
                  <div className="wabi-notes-layout" data-tour="vault-editor">
                    <aside className="wabi-notes-shelf">
                      <div className="wabi-notes-shelf-head">
                        <p className="wabi-notes-count">{vaultNotes.length} in this shelf</p>
                        <button type="button" onClick={startNewVaultNote}>New note</button>
                      </div>
                      {vaultNotes.map((note) => (
                        <button key={note.path} type="button" className={`wabi-note-shelf-row ${vaultNotePath === note.path ? "active" : ""}`} onClick={() => void openVaultNote(note)}>
                          <strong>{note.title}</strong>
                          <span>{new Intl.DateTimeFormat("en", { year: "numeric", month: "short", day: "numeric" }).format(new Date(note.savedAt))}</span>
                        </button>
                      ))}
                      {!vaultNotes.length && !vaultNotesLoading ? <p className="wabi-notes-empty">Create your first note.</p> : null}
                      <div className="wabi-notes-actions">
                        <button type="button" onClick={() => void loadVaultNotes()} disabled={vaultNotesLoading}>{vaultNotesLoading ? "Loading..." : "Refresh"}</button>
                        {vaultDailyEditing ? <button type="button" onClick={() => void handleSaveWabiVaultNote()} disabled={vaultNotesLoading}>Save</button> : <button type="button" onClick={() => setVaultDailyEditing(true)} disabled={!vaultNoteTitle}>Edit</button>}
                        <button type="button" className="wabi-note-delete" onClick={() => void handleDeleteWabiVaultNote()} disabled={!vaultNotePath || vaultNotesLoading}>Delete</button>
                      </div>
                    </aside>
                    <article className="wabi-note-reader">
                      <header>
                        {vaultDailyEditing ? (
                          <input
                            className="wabi-note-title-input"
                            value={vaultNoteTitle}
                            onChange={(event) => {
                              setVaultNoteTitle(event.target.value);
                              setVaultNoteDirty(true);
                            }}
                            placeholder="Note title"
                          />
                        ) : <h2>{vaultNoteTitle || "Untitled note"}</h2>}
                        <p>{vaultNoteDirty ? "unsaved changes" : vaultNoteSavedAt ? new Intl.DateTimeFormat("en", { year: "numeric", month: "short", day: "numeric" }).format(new Date(vaultNoteSavedAt)) : "not saved"}</p>
                      </header>
                      {vaultDailyEditing ? (
                        <textarea
                          className="wabi-note-editor"
                          value={vaultNoteContent}
                          onChange={(event) => {
                            setVaultNoteContent(event.target.value);
                            setVaultNoteDirty(true);
                          }}
                          placeholder="Write your note."
                        />
                      ) : (
                        <div className="markdown-preview wabi-note-prose">{renderMarkdownPreview(vaultNoteContent)}</div>
                      )}
                    </article>
                  </div>
                ) : <div className="vault-space-panel" data-tour="vault-editor">
                  <div className="vault-toolbar">
                    <div className="vault-toolbar-main">
                      <label className="vault-compact-field">
                        <span>Daily</span>
                        <input
                          type="date"
                          data-tour="vault-daily-date"
                          value={vaultNoteDate}
                          onChange={(event) => {
                            setVaultNoteDate(event.target.value || localIsoDate());
                            setVaultNotePath(null);
                            setVaultNoteContent("");
                            setVaultNoteDirty(false);
                            setVaultDailyEditing(false);
                          }}
                        />
                      </label>
                      <span className="vault-path-chip">{vaultNotePath ?? `Daily/${vaultNoteDate}.md`}</span>
                    </div>
                    <div className="vault-toolbar-actions">
                      <button type="button" className="ghost-button" data-tour="vault-load" onClick={() => loadVaultNote()} disabled={vaultNoteLoading}>{vaultNoteLoading ? "Working..." : "Load"}</button>
                      <button type="button" className="ghost-button" data-tour="vault-session-draft" onClick={handleUseGeneratedNote}>Use session draft</button>
                      {vaultDailyEditing ? (
                        <button type="button" className="ghost-button" data-tour="vault-save" onClick={handleSaveVaultNote} disabled={vaultNoteLoading}>Save</button>
                      ) : (
                        <button type="button" className="ghost-button" data-tour="vault-edit" onClick={() => setVaultDailyEditing(true)}>Edit</button>
                      )}
                      <span className="design-chip">{vaultNoteDirty ? "Unsaved" : "Saved"}</span>
                    </div>
                  </div>

                  {vaultDailyEditing ? (
                    <div className="vault-split">
                      <div className="vault-editor-pane">
                        <div className="vault-pane-head"><span className="eyebrow">Editor</span></div>
                        <textarea
                          className="vault-markdown-editor"
                          value={vaultNoteContent || notePreview}
                          onChange={(event) => {
                            setVaultNoteContent(event.target.value);
                            setVaultNoteDirty(true);
                          }}
                          placeholder="Write today's markdown note."
                        />
                      </div>
                      <div className="vault-preview-pane">
                        <div className="vault-pane-head"><span className="eyebrow">Preview</span></div>
                        <div className="markdown-preview vault-prose compact">{renderMarkdownPreview(dailyPreviewContent)}</div>
                      </div>
                    </div>
                  ) : (
                      <div className="vault-document-wrap">
                        <article className="panel-card vault-document">
                          <div className="vault-document-meta">
                            <span>Daily</span>
                            <code>{vaultNotePath ?? `Daily/${vaultNoteDate}.md`}</code>
                          </div>
                          <div className="markdown-preview vault-prose">{renderMarkdownPreview(dailyPreviewContent)}</div>
                        </article>

                        {appStyle !== "field-notebook" ? <div className="vault-recent-list" data-tour="vault-recent">
                          <p className="eyebrow">Recent exports</p>
                          {state.exports.length ? state.exports.slice(0, 4).map((item) => (
                            <div key={item.id} className="vault-recent-row">
                            <span>{item.noteDate}</span>
                            <code>{item.notePath}</code>
                            </div>
                          )) : <p className="empty-copy">Exports will appear here once you save a daily note.</p>}
                        </div> : null}
                      </div>
                  )}
                </div>
              ) : vaultSpace === "references" ? (
                <div className={`vault-space-panel ${appStyle === "wabi-sabi" ? "wabi-vault-tabbed" : ""}`}>
                  {appStyle === "wabi-sabi" ? renderWabiVaultCourseShelf("references") : null}
                  <div className="vault-toolbar">
                    {appStyle !== "field-notebook" ? <div className="vault-toolbar-main references-toolbar-main">
                      <label className="vault-compact-field">
                        <span>Semester</span>
                        <select
                          value={referenceSemesterId}
                          onChange={(event) => {
                            if (referenceDirty) {
                              setMessage("Save the current reference note before switching courses.");
                              return;
                            }
                            setReferenceSemesterId(event.target.value);
                            setReferenceCourseId("");
                          }}
                        >
                          {activeSemesters.map((semester) => <option key={semester.id} value={semester.id}>{semester.name}</option>)}
                        </select>
                      </label>
                      <div className="vault-course-chips">
                        {referenceCourses.map((course) => (
                          <button
                            key={course.id}
                            type="button"
                            className={`vault-course-chip ${course.id === referenceCourseId ? "active" : ""}`}
                            onClick={() => {
                              if (referenceDirty) {
                                setMessage("Save the current reference note before switching courses.");
                                return;
                              }
                              setReferenceCourseId(course.id);
                            }}
                          >
                            <span style={{ background: course.color }} />{course.name}
                          </button>
                        ))}
                      </div>
                    </div> : <div className="vault-toolbar-main references-toolbar-main fn-vault-selection-title">
                      <p className="eyebrow">References</p>
                      <h2>{selectedReferenceCourse ? selectedReferenceCourse.name : "Choose a course"}</h2>
                    </div>}
                    <div className="vault-toolbar-actions">
                      {referenceEditing ? (
                        <button type="button" className="ghost-button" onClick={handleSaveReferenceNote} disabled={referenceLoading || !selectedReferenceCourse}>{referenceLoading ? "Saving..." : "Save"}</button>
                      ) : (
                        <button type="button" className="ghost-button" onClick={handleEditReferenceNote} disabled={!selectedReferenceCourse}>Edit</button>
                      )}
                      <span className="design-chip">{referenceDirty ? "Unsaved" : "Saved"}</span>
                    </div>
                  </div>

                  {selectedReferenceSemester && selectedReferenceCourse ? (
                    referenceEditing ? (
                      <div className="vault-split">
                        <div className="vault-editor-pane">
                          <div className="vault-pane-head"><span className="eyebrow">Markdown</span></div>
                          <textarea
                            className="vault-markdown-editor"
                            value={referenceContent}
                            onChange={(event) => {
                              setReferenceContent(event.target.value);
                              setReferenceDirty(true);
                            }}
                            placeholder="Add useful markdown links for this course."
                          />
                        </div>
                        <div className="vault-preview-pane">
                          <div className="vault-pane-head"><span className="eyebrow">Preview</span></div>
                          <div className="markdown-preview vault-prose compact">{renderMarkdownPreview(referencePreview)}</div>
                        </div>
                      </div>
                    ) : (
                      <div className="vault-document-wrap">
                        <article className="panel-card vault-document">
                          <div className="vault-document-meta">
                            <button type="button" className="vault-document-meta-button" onClick={() => setReferencePathVisible((current) => !current)}>
                              References
                            </button>
                            {referencePathVisible ? <code>{referencePath ?? `References/${selectedReferenceSemester.name}/${selectedReferenceCourse.name}.md`}</code> : null}
                          </div>
                          <div className="markdown-preview vault-prose">{renderMarkdownPreview(referencePreview)}</div>
                        </article>
                      </div>
                    )
                  ) : (
                    <article className="panel-card vault-empty-card">
                      <p className="eyebrow">No courses yet</p>
                      <h2>References need courses</h2>
                      <p className="section-note">Create at least one semester and course in Planner before adding course reference notes.</p>
                    </article>
                  )}
                </div>
              ) : (
                <div className={`vault-space-panel ${appStyle === "wabi-sabi" ? "wabi-vault-tabbed" : ""}`}>
                  {appStyle === "wabi-sabi" ? renderWabiVaultCourseShelf("summaries") : null}
                  <div className="vault-toolbar">
                    {appStyle !== "field-notebook" ? <div className="vault-toolbar-main references-toolbar-main">
                      <label className="vault-compact-field">
                        <span>Semester</span>
                        <select
                          value={summarySemesterId}
                          onChange={(event) => {
                            setSummarySemesterId(event.target.value);
                            setSummaryCourseId("");
                          }}
                        >
                          {activeSemesters.map((semester) => <option key={semester.id} value={semester.id}>{semester.name}</option>)}
                        </select>
                      </label>
                      <div className="vault-course-chips">
                        {summaryCourses.map((course) => (
                          <button
                            key={course.id}
                            type="button"
                            className={`vault-course-chip ${course.id === summaryCourseId ? "active" : ""}`}
                            onClick={() => setSummaryCourseId(course.id)}
                          >
                            <span style={{ background: course.color }} />{course.name}
                          </button>
                        ))}
                      </div>
                    </div> : <div className="vault-toolbar-main references-toolbar-main fn-vault-selection-title">
                      <p className="eyebrow">Summaries</p>
                      <h2>{selectedSummaryCourse ? selectedSummaryCourse.name : "Choose a course"}</h2>
                    </div>}
                    {appStyle !== "field-notebook" ? <div className="vault-toolbar-actions">
                      <button type="button" className="ghost-button" onClick={() => loadSummaryFileList()} disabled={summaryLoading || !selectedSummaryCourse}>{summaryLoading ? "Loading..." : "Refresh"}</button>
                      <button type="button" onClick={handleAddSummaryFiles} disabled={summaryLoading || !selectedSummaryCourse}>Add files</button>
                    </div> : null}
                  </div>

                  {selectedSummarySemester && selectedSummaryCourse ? (
                    <div className="summaries-layout">
                      {appStyle !== "field-notebook" && (appStyle !== "wabi-sabi" || summaryFiles.length > 1) ? <aside className="panel-card summary-file-list">
                        <div className="summary-file-list-head">
                          <div>
                            <p className="eyebrow">Summaries</p>
                            <h2>{selectedSummaryCourse.name}</h2>
                          </div>
                          <span>{summaryFiles.length}</span>
                        </div>
                        {summaryFiles.length ? (
                          <div className="summary-file-links">
                            {summaryFiles.map((file) => (
                              <button
                                key={file.path}
                                type="button"
                                className={`summary-file-link ${file.path === selectedSummaryFile?.path ? "active" : ""}`}
                                onClick={() => setSelectedSummaryPath(file.path)}
                              >
                                <span>{file.name}</span>
                                <small>{file.kind.toUpperCase()} • {formatFileSize(file.sizeBytes)}</small>
                              </button>
                            ))}
                          </div>
                        ) : (
                          <p className="empty-copy">No summaries yet. Add PDFs, formula sheets, or cheatsheet images for this course.</p>
                        )}
                      </aside> : null}

                      <section className="panel-card summary-viewer-card">
                        {selectedSummaryFile && selectedSummaryUrl ? (
                          <>
                            <div className="summary-viewer-head">
                              <div>
                                <p className="eyebrow">Viewer</p>
                                <h2>{selectedSummaryFile.name}</h2>
                              </div>
                              <button type="button" className="ghost-button small-button" onClick={() => revealItemInDir(selectedSummaryFile.path)}>Show file</button>
                            </div>
                            {selectedSummaryFile.kind === "pdf" ? (
                              <Suspense fallback={<p className="summary-pdf-status">Preparing PDF viewer...</p>}>
                                <SummaryPdfViewer vaultPath={state.settings.vaultPath} path={selectedSummaryFile.path} title={selectedSummaryFile.name} />
                              </Suspense>
                            ) : (
                              <div className="summary-image-viewer">
                                <img src={selectedSummaryUrl} alt={selectedSummaryFile.name} />
                              </div>
                            )}
                          </>
                        ) : (
                          <div className="summary-viewer-empty">
                            <p className="eyebrow">Viewer</p>
                            <h2>Select a summary</h2>
                            <p className="section-note">Choose a file from the list, or add PDFs/images to this course.</p>
                          </div>
                        )}
                      </section>
                    </div>
                  ) : (
                    <article className="panel-card vault-empty-card">
                      <p className="eyebrow">No courses yet</p>
                      <h2>Summaries need courses</h2>
                      <p className="section-note">Create at least one semester and course in Planner before adding summary files.</p>
                    </article>
                  )}
                </div>
              )}
            </>
          ) : null}
        </section>
  );
});
