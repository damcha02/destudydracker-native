import { useEffect, useRef, useState } from "react";
import type { PDFDocumentProxy } from "pdfjs-dist";
import { readSummaryPdf } from "../lib/obsidian";

type PdfJsModule = typeof import("pdfjs-dist");

function getErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return fallback;
}

export function SummaryPdfViewer({ vaultPath, path, title }: { vaultPath: string; path: string; title: string }) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const renderTaskRef = useRef<{ cancel: () => void } | null>(null);
  const [documentProxy, setDocumentProxy] = useState<PDFDocumentProxy | null>(null);
  const [pageNumber, setPageNumber] = useState(1);
  const [pageCount, setPageCount] = useState(0);
  const [scale, setScale] = useState(1.25);
  const [loading, setLoading] = useState(false);
  const [rendering, setRendering] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let loadingTask: ReturnType<PdfJsModule["getDocument"]> | null = null;
    setLoading(true);
    setError(null);
    setDocumentProxy(null);
    setPageNumber(1);
    setPageCount(0);

    void Promise.all([
      import("pdfjs-dist"),
      import("pdfjs-dist/build/pdf.worker.mjs?url"),
      readSummaryPdf(vaultPath, path),
    ])
      .then(([pdfjsLib, workerModule, bytes]) => {
        if (cancelled) return null;
        pdfjsLib.GlobalWorkerOptions.workerSrc = workerModule.default;
        loadingTask = pdfjsLib.getDocument({ data: new Uint8Array(bytes) });
        return loadingTask.promise;
      })
      .then((pdf) => {
        if (!pdf) return;
        if (cancelled) {
          void pdf.cleanup();
          return;
        }
        setDocumentProxy(pdf);
        setPageCount(pdf.numPages);
      })
      .catch((loadError: unknown) => {
        if (!cancelled) setError(getErrorMessage(loadError, "Could not load this PDF."));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
      renderTaskRef.current?.cancel();
      renderTaskRef.current = null;
      void loadingTask?.destroy();
    };
  }, [path, vaultPath]);

  useEffect(() => {
    if (!documentProxy || !canvasRef.current) return undefined;
    let cancelled = false;
    setRendering(true);
    setError(null);
    renderTaskRef.current?.cancel();

    void documentProxy.getPage(pageNumber)
      .then((page) => {
        if (cancelled || !canvasRef.current) return;
        const canvas = canvasRef.current;
        const context = canvas.getContext("2d");
        if (!context) throw new Error("Could not prepare PDF canvas.");

        const viewport = page.getViewport({ scale });
        const pixelRatio = window.devicePixelRatio || 1;
        canvas.width = Math.floor(viewport.width * pixelRatio);
        canvas.height = Math.floor(viewport.height * pixelRatio);
        canvas.style.width = `${viewport.width}px`;
        canvas.style.height = `${viewport.height}px`;
        context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
        context.clearRect(0, 0, viewport.width, viewport.height);

        const renderTask = page.render({ canvas, canvasContext: context, viewport });
        renderTaskRef.current = renderTask;
        return renderTask.promise;
      })
      .catch((renderError: unknown) => {
        if (cancelled) return;
        if (renderError instanceof Error && renderError.name === "RenderingCancelledException") return;
        setError(getErrorMessage(renderError, "Could not render this PDF page."));
      })
      .finally(() => {
        if (!cancelled) setRendering(false);
      });

    return () => {
      cancelled = true;
      renderTaskRef.current?.cancel();
      renderTaskRef.current = null;
    };
  }, [documentProxy, pageNumber, scale]);

  return (
    <div className="summary-pdf-viewer">
      <div className="summary-pdf-controls">
        <button type="button" className="ghost-button small-button" onClick={() => setPageNumber((current) => Math.max(1, current - 1))} disabled={!documentProxy || pageNumber <= 1}>Prev</button>
        <span>Page {pageCount ? pageNumber : "-"} / {pageCount || "-"}</span>
        <button type="button" className="ghost-button small-button" onClick={() => setPageNumber((current) => Math.min(pageCount, current + 1))} disabled={!documentProxy || pageNumber >= pageCount}>Next</button>
        <button type="button" className="ghost-button small-button" onClick={() => setScale((current) => Math.max(0.7, Number((current - 0.15).toFixed(2))))} disabled={!documentProxy}>-</button>
        <span>{Math.round(scale * 100)}%</span>
        <button type="button" className="ghost-button small-button" onClick={() => setScale((current) => Math.min(2.5, Number((current + 0.15).toFixed(2))))} disabled={!documentProxy}>+</button>
      </div>
      <div className="summary-pdf-canvas-wrap" aria-busy={loading || rendering}>
        {loading ? <p className="summary-pdf-status">Loading PDF...</p> : null}
        {error ? <p className="summary-pdf-error">{error}</p> : null}
        <canvas ref={canvasRef} aria-label={title} />
        {rendering && !loading ? <p className="summary-pdf-status floating">Rendering page...</p> : null}
      </div>
    </div>
  );
}
