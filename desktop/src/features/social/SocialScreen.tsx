/* eslint-disable @typescript-eslint/ban-ts-comment, @typescript-eslint/no-explicit-any */
// @ts-nocheck -- exact JSX bridge; social state ownership remains in App.
// This is an exact UI extraction; state and actions remain owned by App.
import { useState } from "react";
import { createPortal } from "react-dom";

type Props = Record<string, unknown>;

export function SocialScreen(props: Props) {
  // Wabi-Sabi: clicking a small profile picture in the feed or standings shows it larger.
  const [expandedAvatarHtml, setExpandedAvatarHtml] = useState<string | null>(null);
  const { AVATAR_CROP_MAX_ZOOM, AVATAR_CROP_VIEWPORT_PX, ArenaAvatar, ArenaBg, ArenaLeaderboardRow, ArenaRankBadge, MAX_FEED_POLL_OPTIONS, SQUAD_SEASON_NAME, SQUAD_SEASON_RANGE_LABEL, SQUAD_TRACKING_START_LABEL, SquadArenaRow, addFeedPollOption, alphabetLetters, answerFriendRequest, answerSquadRequest, appStyle, avatarCrop, avatarCropDragging, avatarCropOpen, avatarCropSource, avatarIcons, avatarStyles, badgesOpen, canKickSquadMember, canManageCurrentSquadRequests, canViewR2Usage, cancelAvatarCrop, cancelEditingFeedPost, cancelEditingSocialName, changeSquadMemberRole, clearFeedPollDraft, closeProfileAvatarEditor, confirmAvatarCrop, copyFriendCode, copyFriendInviteLink, currentSquad, currentSquadRole, deleteOwnFeedPost, deleteOwnSquadMessage, displayedSocialFeed, editingFeedPostId, editingFeedPostImage, editingFeedPostNote, editingFeedPostRemoveImage, emojiPickerPostId, expandedFeedComments, expandedFeedImage, expandedSquadMemberId, failedFeedImages, feedCommentDrafts, feedCommentSavingId, feedImageDraft, feedImagesVisible, feedLoading, feedNoteDraft, feedPollDraft, feedPollHasDraft, feedPollPanelOpen, feedPollsVisible, feedPostSaving, feedScope, formatBytes, formatCompactNumber, formatFeedPostedAt, formatMinutes, formatProfileSeenAt, friendCodeDraft, friendInviteLink, getAssignableSquadRoles, getFirstAvatarLetter, handleAvatarCropPointerDown, handleAvatarCropPointerEnd, handleAvatarCropPointerMove, handleAvatarCropWheel, handleAvatarCropZoomChange, handleEditingFeedPostImageChange, handleFeedImageDraftChange, handleProfileAvatarPhotoChange, incomingFriendRequestCount, isRecentlyActive, joinOrRequestSquad, joinOrRequestViewedSquad, kickFromSquad, lastSocialSyncLabel, latestFeedSession, latestFeedSessionPosted, leaveCurrentSquad, liveFriends, loadSquadSuggestions, localSocialDaily, localSocialMonthly, localSocialOverall, localSocialWeekly, myFriendRank, myGlobalRank, openFriendProfile, openProfileAvatarEditor, openSquadDetails, postLatestSessionToFeed, profileAvatarDraft, profileAvatarEditorOpen, profileAvatarFileInputRef, profileAvatarLetterPickerOpen, profileBadgeGroups, r2UsageStatus, removeFeedPollOption, renderProfileBadgeCard, runSocialSync, saveFeedPostEdit, saveProfileAvatar, saveSocialName, sendFriendRequestToCode, setBadgesOpen, setEditingFeedPostImage, setEditingFeedPostNote, setEditingFeedPostRemoveImage, setEmojiPickerPostId, setExpandedFeedImageId, setExpandedSquadMemberId, setFailedFeedImages, setFeedCommentDrafts, setFeedImageDraft, setFeedNoteDraft, setFeedPollDraft, setFeedPollPanelOpen, setFeedScope, setFriendCodeDraft, setProfileAvatarDraft, setProfileAvatarLetterPickerOpen, setSocialNameDraft, setSocialPeriod, setSocialScope, setSocialSubtab, setSquadChatDraft, setSquadNameDraft, setSquadPrivateDraft, setSquadScorePeriod, setSquadSearchDraft, setSquadSettingsEditing, setSquadSettingsNameDraft, setSquadSettingsPrivateDraft, setViewingFriend, setViewingSquadDetails, setViewingSquadEntry, setWabiCircleCompetitive, socialArenaSubtitle, socialArenaTitle, socialConfigured, socialLeaderboard, socialLeaderboardTopMinutes, socialNameDraft, socialNameEditing, socialPeriod, socialScope, socialSubtab, socialSubtabs, socialSyncing, squadChatDraft, squadMemberLeaderboard, squadNameDraft, squadPrivateDraft, squadRoleLabels, squadScoreLeaderboard, squadScorePeriod, squadSearchDraft, squadSearchResults, squadSearching, squadSettingsEditing, squadSettingsNameDraft, squadSettingsPrivateDraft, squadSuggestionPool, squadSuggestions, squadSuggestionsLoading, startEditingFeedPost, startEditingSocialName, startSquadSettingsEdit, state, submitFeedComment, submitFriendRequest, submitSquadChat, submitSquadCreate, submitSquadSearch, submitSquadSettings, toggleAutoPostSessions, toggleFeedComments, toggleLocalFeedReaction, toggleProfilePrivacy, toggleShowHoursToFriends, updateFeedPollOption, viewingFriend, viewingFriendLoading, viewingFriendStats, viewingIsFriend, viewingIsSelf, viewingRequestPending, viewingSquadAction, viewingSquadDetails, viewingSquadEntry, viewingSquadLoading, voteFeedPoll, wabiAttendanceFriends, wabiCircleCompetitive, weekCompareEntries } = props as any;
  return (
    <>
      {state.activeTab === "friends" ? (
        <section
          className={`arena-root fade-up ${appStyle === "field-notebook" ? "study-circle-root" : appStyle === "wabi-sabi" ? "wabi-circle-root" : ""}`}
          onClickCapture={appStyle === "wabi-sabi" ? (event) => {
            const avatar = (event.target as HTMLElement).closest(".wabi-feed-card .arena-avatar, .wabi-standings .arena-avatar");
            if (!avatar) return;
            event.stopPropagation();
            event.preventDefault();
            setExpandedAvatarHtml(avatar.outerHTML);
          } : undefined}
        >
          {appStyle === "wabi-sabi" && expandedAvatarHtml ? createPortal(
            // Portaled to <body>: inside the page, an animated ancestor made "fixed" relative to the whole
            // (tall) page, so the picture opened somewhere you had to scroll to.
            <div className="wabi-avatar-lightbox" role="dialog" aria-label="Profile picture" onClick={() => setExpandedAvatarHtml(null)}>
              <div className="wabi-avatar-lightbox-card" dangerouslySetInnerHTML={{ __html: expandedAvatarHtml }} />
            </div>,
            document.body,
          ) : null}
          {appStyle === "modern" ? <ArenaBg /> : null}
          {appStyle === "wabi-sabi" && socialSubtab !== "feed" ? null : (
          <div className="arena-hero-header">
            <span className="arena-hero-title">{appStyle === "field-notebook" ? "Study Circle" : appStyle === "wabi-sabi" ? "Circle" : "Study Arena"}</span>
            <span className="arena-hero-sub">{appStyle === "field-notebook" ? "Attendance sheet · friends · squads · quiet accountability" : appStyle === "wabi-sabi" ? "Who is sitting down today. Attendance, not a leaderboard." : "Compete. Focus. Rise."}</span>
          </div>
          )}

          {appStyle === "wabi-sabi" ? null : <nav className="social-nav" aria-label="Social spaces" data-tour="social-nav">
            {socialSubtabs.filter((space) => appStyle !== "wabi-sabi" || space.id !== "leaderboard" || wabiCircleCompetitive).map((space) => {
              const active = space.id === socialSubtab;
              return (
                <button key={space.id} type="button" data-tour={`social-${space.id}-tab`} className={`social-nav-item ${active ? "active" : ""}`} onClick={() => setSocialSubtab(space.id)}>
                  {appStyle === "wabi-sabi" && space.id === "leaderboard" ? "Standings" : space.label}
                  {space.id === "friends" && incomingFriendRequestCount > 0 ? (
                    <span className="social-nav-request-badge" aria-label={`${incomingFriendRequestCount} incoming friend request${incomingFriendRequestCount === 1 ? "" : "s"}`}>
                      {incomingFriendRequestCount > 9 ? "9+" : incomingFriendRequestCount}
                    </span>
                  ) : null}
                  {space.badge ? <span className="social-nav-item-badge">{space.badge}</span> : null}
                </button>
              );
            })}
          </nav>}

          {appStyle === "wabi-sabi" && socialSubtab === "feed" ? (
            <div className="wabi-attendance" data-tour="social-live">
              {wabiAttendanceFriends.length ? (
                wabiAttendanceFriends.map((friend) => {
                  const sitting = isRecentlyActive(friend.lastSeenAt);
                  return (
                    <div key={friend.userId} className="wabi-attendance-row" onClick={() => void openFriendProfile(friend)} role="button" tabIndex={0} onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); openFriendProfile(friend); } }}>
                      <span className="wabi-attendance-name">{friend.displayName}</span>
                      <span className={`wabi-attendance-state ${sitting ? "sitting" : "resting"}`}>{sitting ? "sitting" : "resting"}</span>
                    </div>
                  );
                })
              ) : (
                <p className="wabi-empty">No friends seen in the last 48 hours. Add friends from the Friends tab below.</p>
              )}
            </div>
          ) : null}

          {socialSubtab === "feed" ? (
            <div className={`social-feed-shell ${appStyle === "wabi-sabi" ? "wabi-feed-shell" : ""}`}>
              <div className="social-feed-sidebar">
                <div className="arena-scope-toggle social-feed-scope" aria-label="Feed scope" data-tour="social-feed-scope">
                  {(["friends", "global"] as SocialFeedScope[]).map((scope) => (
                    <button key={scope} type="button" className={feedScope === scope ? "arena-scope-btn arena-scope-btn--active" : "arena-scope-btn"} onClick={() => setFeedScope(scope)}>
                      {appStyle === "field-notebook" ? (scope === "global" ? "Global log" : "Friends log") : (scope === "global" ? "Global Feed" : "Friends Feed")}
                    </button>
                  ))}
                  <button type="button" className="arena-btn arena-btn--decline social-refresh-btn" data-tour="social-refresh" onClick={() => void runSocialSync()} disabled={socialSyncing || !socialConfigured}>
                    {socialSyncing ? "Syncing..." : "Refresh"}
                  </button>
                </div>

                <div className="stories" aria-label="Study circle stories" data-tour="social-stories">
                  <div className="story">
                    <div className="story__ring story__ring--self"><ArenaAvatar name={state.social.displayName} avatar={state.social.avatar} self size="md" /></div>
                    <span>You</span>
                  </div>
                  {state.social.friends.slice(0, 10).map((friend) => (
                    <div key={friend.userId} className="story" onClick={() => void openFriendProfile(friend)} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openFriendProfile(friend); } }}>
                      <div className={`story__ring ${isRecentlyActive(friend.lastSeenAt) ? "" : "story__ring--idle"}`}><ArenaAvatar name={friend.displayName} avatar={friend.avatar} size="md" /></div>
                      <span>{friend.displayName}</span>
                    </div>
                  ))}
                </div>

                <div className="live-bar" data-tour="social-live">
                  <span className="live-dot" />
                  <strong>{liveFriends.length ? `${liveFriends.length} friends` : "No friends"}</strong>
                  <span>recently active</span>
                  <small>{liveFriends.slice(0, 3).map((friend) => friend.displayName).join(" · ") || "Sync to update live status"}</small>
                </div>

                {canViewR2Usage && (r2UsageStatus?.warning || r2UsageStatus?.paused) ? (
                  <div className={`feed-r2-safety ${r2UsageStatus.paused ? "feed-r2-safety--paused" : ""}`}>
                    <strong>{r2UsageStatus.paused ? "Image uploads paused" : "Image usage close to safety limit"}</strong>
                    <span>
                      Storage {formatBytes(r2UsageStatus.storageBytes)} / {formatBytes(r2UsageStatus.limits.storageHardBytes)} · Writes {formatCompactNumber(r2UsageStatus.classAOps)} / {formatCompactNumber(r2UsageStatus.limits.classAHardMonthly)} · Reads {formatCompactNumber(r2UsageStatus.classBOps)} / {formatCompactNumber(r2UsageStatus.limits.classBHardMonthly)}
                    </span>
                    <small>These strict app limits are far below Cloudflare R2's free tier to avoid overage risk.</small>
                  </div>
                ) : null}

                <form className={`feed-composer ${appStyle === "wabi-sabi" ? "wabi-feed-composer" : ""}`} data-tour="social-composer" onSubmit={postLatestSessionToFeed}>
                  <div>
                    <span className="arena-kicker">{appStyle === "field-notebook" ? "Post your last block" : appStyle === "wabi-sabi" ? "Circle note" : "Share latest session"}</span>
                    <h3>{latestFeedSession ? appStyle === "wabi-sabi" ? "Say something to the circle" : `${formatMinutes(latestFeedSession.minutes)} ${latestFeedSession.kind} block` : "No session ready"}</h3>
                    <p>{latestFeedSession ? (appStyle === "field-notebook" ? "One line for the circle, or leave it blank." : appStyle === "wabi-sabi" ? "A note will accompany your latest study block." : "Write one sentence, or leave it blank for a chaotic default.") : "Finish a study or exam block, then publish it here."}</p>
                  </div>
                  <input className="arena-input" data-tour="social-note" value={feedNoteDraft} onChange={(event) => setFeedNoteDraft(event.target.value)} placeholder={appStyle === "field-notebook" ? "one line, or leave it blank..." : appStyle === "wabi-sabi" ? "say something to the circle, then Enter" : "one sentence for the feed..."} disabled={!latestFeedSession || latestFeedSessionPosted} />
                  <div className="feed-composer-actions">
                    <label className="feed-action-icon" data-tour="social-image" title={feedImageDraft ? "Change image" : "Add image"} aria-label={feedImageDraft ? "Change image" : "Add image"}>
                      <input type="file" accept="image/png,image/jpeg,image/webp,image/gif" onChange={(event) => void handleFeedImageDraftChange(event)} disabled={!latestFeedSession || latestFeedSessionPosted || (canViewR2Usage && r2UsageStatus?.paused)} />
                      <span aria-hidden="true">▧</span>
                    </label>
                    <button type="button" className={`feed-action-icon ${feedPollHasDraft ? "feed-action-icon--active" : ""}`} data-tour="social-poll" onClick={() => setFeedPollPanelOpen((open) => !open)} disabled={!latestFeedSession || latestFeedSessionPosted} title="Create poll" aria-label="Create poll">◉</button>
                  </div>
                  <button type="submit" className="arena-btn arena-btn--send" data-tour="social-post" disabled={!latestFeedSession || latestFeedSessionPosted}>{latestFeedSessionPosted ? "Posted" : "Post"}</button>
                  {feedPollPanelOpen ? (
                    <div className="feed-poll-popover">
                      <div className="feed-poll-popover__head">
                        <strong>Create poll</strong>
                        <label className="feed-poll-switch">
                          <span>Multiple answers</span>
                          <input type="checkbox" checked={feedPollDraft.multiple} onChange={(event) => setFeedPollDraft((current) => ({ ...current, multiple: event.target.checked }))} />
                          <i className="ios-switch" aria-hidden="true" />
                        </label>
                      </div>
                      <input className="arena-input" value={feedPollDraft.question} onChange={(event) => setFeedPollDraft((current) => ({ ...current, question: event.target.value }))} placeholder="Question" maxLength={180} />
                      <div className="feed-poll-options-editor">
                        {feedPollDraft.options.map((option, index) => (
                          <div key={index} className="feed-poll-option-editor">
                            <input className="arena-input" value={option} onChange={(event) => updateFeedPollOption(index, event.target.value)} placeholder={`Option ${index + 1}`} maxLength={100} />
                            {feedPollDraft.options.length > 2 ? <button type="button" className="feed-poll-option-remove" onClick={() => removeFeedPollOption(index)} aria-label={`Remove option ${index + 1}`}>×</button> : null}
                          </div>
                        ))}
                      </div>
                      <button type="button" className="feed-poll-add-option" onClick={addFeedPollOption} disabled={feedPollDraft.options.length >= MAX_FEED_POLL_OPTIONS}>+ Add option</button>
                      <div className="feed-poll-popover__actions">
                        <button type="button" className="arena-btn arena-btn--send" onClick={() => setFeedPollPanelOpen(false)}>Done</button>
                        <button type="button" className="ghost-button small-button" onClick={clearFeedPollDraft}>Clear</button>
                      </div>
                    </div>
                  ) : null}
                  {feedImageDraft ? (
                    <div className="feed-image-draft">
                      <img src={feedImageDraft.previewUrl} alt="Selected feed post preview" />
                      <button type="button" className="ghost-button small-button" onClick={() => setFeedImageDraft((current) => { if (current) URL.revokeObjectURL(current.previewUrl); return null; })}>Remove image</button>
                    </div>
                  ) : null}
                </form>
              </div>

              <div className={`social-feed-main ${appStyle === "wabi-sabi" ? "wabi-feed-main" : ""}`}>
              <div className="section-label" data-tour="social-feed">{appStyle === "field-notebook" ? "Circle log" : appStyle === "wabi-sabi" ? "Feed" : "Activity"} {feedLoading ? "· Refreshing" : ""}</div>
              {displayedSocialFeed.length ? displayedSocialFeed.map((item) => {
                const isOwnPost = item.userId === state.social.userId || item.isSelf;
                const profileTarget = { userId: item.userId, displayName: item.displayName, friendCode: item.friendCode, avatar: isOwnPost ? state.social.avatar : item.avatar };
                const comments = item.comments ?? [];
                const commentsOpen = expandedFeedComments.has(item.id);
                return item.type === "milestone" ? (
                  <article key={item.id} className="milestone">
                    <div className="milestone__icon">{item.icon || "🏆"}</div>
                    <div>
                      <h3><button type="button" className="social-name-button" onClick={() => void openFriendProfile(profileTarget)}>{item.displayName}</button> hit a milestone</h3>
                      <p>{item.note || item.detail}</p>
                    </div>
                  </article>
                ) : (
                  <article key={item.id} className={`feed-card ${appStyle === "wabi-sabi" ? "wabi-feed-card" : ""}`}>
                    <div className="feed-card__head">
                      <ArenaAvatar name={item.displayName} avatar={isOwnPost ? state.social.avatar : item.avatar} self={isOwnPost} />
                      <div>
                        <button type="button" className="social-name-button social-name-button--strong" onClick={() => void openFriendProfile(profileTarget)}>{item.displayName}{isOwnPost ? " (You)" : ""}</button>
                        <span>{formatFeedPostedAt(item.createdAt)}</span>
                      </div>
                      {isOwnPost ? <button type="button" className="arena-icon-button feed-edit-button" onClick={() => startEditingFeedPost(item)} title="Edit post">✎</button> : null}
                    </div>
                    <div className="feed-card__body">
                      <div className="feed-card__session">
                        <span>{item.icon || "✦"}</span>
                        <div>
                          <strong>{item.subject || "Study session"}</strong>
                          <small>{item.detail || `${formatMinutes(item.minutes)} · ${item.presetLabel || "Focus"}`}</small>
                        </div>
                      </div>
                      {editingFeedPostId === item.id ? (
                        <div className="feed-edit-panel">
                          <textarea className="arena-input feed-edit-textarea" value={editingFeedPostNote} onChange={(event) => setEditingFeedPostNote(event.target.value)} maxLength={220} autoFocus />
                          <div className="feed-image-edit-row">
                            <label className="feed-image-picker">
                              <input type="file" accept="image/png,image/jpeg,image/webp,image/gif" onChange={(event) => void handleEditingFeedPostImageChange(event)} disabled={feedPostSaving || (canViewR2Usage && Boolean(r2UsageStatus?.paused)) || state.social.pendingFeedPosts.some((post) => post.id === item.id)} />
                              <span>{item.imageUrl || item.imageExpiredAt || editingFeedPostImage ? "Replace image" : "Add image"}</span>
                            </label>
                            {item.imageUrl && !editingFeedPostImage ? <button type="button" className="ghost-button small-button" onClick={() => setEditingFeedPostRemoveImage(true)} disabled={feedPostSaving}>Remove image</button> : null}
                          </div>
                          {state.social.pendingFeedPosts.some((post) => post.id === item.id) ? <p className="feed-image-hint">Sync this post before adding an image.</p> : null}
                          {editingFeedPostImage ? (
                            <div className="feed-image-draft feed-image-draft--edit">
                              <img src={editingFeedPostImage.previewUrl} alt="Replacement feed post preview" />
                              <button type="button" className="ghost-button small-button" onClick={() => setEditingFeedPostImage((current) => { if (current) URL.revokeObjectURL(current.previewUrl); return null; })}>Clear replacement</button>
                            </div>
                          ) : editingFeedPostRemoveImage ? <p className="feed-image-hint">Image will be removed when you save.</p> : null}
                          <div className="feed-edit-actions">
                            <button type="button" className="arena-btn arena-btn--send" onClick={() => void saveFeedPostEdit(item.id)} disabled={feedPostSaving}>Save</button>
                            <button type="button" className="ghost-button small-button" onClick={cancelEditingFeedPost} disabled={feedPostSaving}>Cancel</button>
                            <button type="button" className="arena-btn arena-btn--decline" onClick={() => void deleteOwnFeedPost(item.id)} disabled={feedPostSaving}>Delete</button>
                          </div>
                        </div>
                      ) : item.note ? <p>"{item.note}"</p> : null}
                      {feedPollsVisible && item.poll ? (
                        <div className="feed-poll-card">
                          <div className="feed-poll-card__head">
                            <strong>{item.poll.question}</strong>
                            <span>{item.poll.multiple ? "Multiple answers" : "One answer"}</span>
                          </div>
                          <div className="feed-poll-card__options">
                            {item.poll.options.map((option) => {
                              const percent = item.poll?.totalVotes ? Math.round((option.votes / item.poll.totalVotes) * 100) : 0;
                              return (
                                <button key={option.id} type="button" className={`feed-poll-vote ${option.selected ? "feed-poll-vote--selected" : ""}`} onClick={() => void voteFeedPoll(item, option.id)}>
                                  <span className="feed-poll-vote__fill" style={{ width: `${percent}%` }} />
                                  <span className="feed-poll-vote__check">{option.selected ? "✓" : item.poll?.multiple ? "□" : "○"}</span>
                                  <span className="feed-poll-vote__text">{option.text}</span>
                                  <span className="feed-poll-vote__count">{option.votes} · {percent}%</span>
                                </button>
                              );
                            })}
                          </div>
                          <small>{item.poll.totalVotes} vote{item.poll.totalVotes === 1 ? "" : "s"}</small>
                        </div>
                      ) : null}
                      {feedImagesVisible && item.imageUrl && !failedFeedImages.has(item.id) ? (
                        <button type="button" className="feed-card__image" onClick={() => setExpandedFeedImageId(item.id)} aria-label="Open feed image fullscreen">
                          <img src={`${item.imageUrl}${item.imageUrl.includes("?") ? "&" : "?"}v=${encodeURIComponent(item.imageExpiresAt ?? item.createdAt)}`} alt={`${item.displayName}'s feed post image`} loading="lazy" onLoad={() => setFailedFeedImages((current) => { if (!current.has(item.id)) return current; const next = new Set(current); next.delete(item.id); return next; })} onError={() => setFailedFeedImages((current) => new Set(current).add(item.id))} />
                        </button>
                      ) : feedImagesVisible && item.imageUrl && failedFeedImages.has(item.id) ? <p className="feed-card__image-expired">Image could not load</p> : feedImagesVisible && item.imageExpiredAt ? <p className="feed-card__image-expired">Image expired</p> : null}
                    </div>
                    <div className="feed-card__reactions">
                      {(() => {
                        const emojiKeys = appStyle === "wabi-sabi"
                          ? ["fire"]
                          : ["fire", "brain", "clap", ...Object.keys(item.reactions ?? {}).filter((k) => k !== "fire" && k !== "brain" && k !== "clap" && (item.reactions?.[k] ?? 0) > 0)];
                        const seen = new Set<string>();
                        return emojiKeys.filter((k) => { if (seen.has(k)) return false; seen.add(k); return true; }).map((emojiKey) => {
                          const count = item.reactions?.[emojiKey] ?? 0;
                          if (count === 0 && emojiKey !== "fire" && emojiKey !== "brain" && emojiKey !== "clap") return null;
                          const display = appStyle === "wabi-sabi" ? "Nod" : emojiKey === "fire" ? "🔥" : emojiKey === "brain" ? "🧠" : emojiKey === "clap" ? "👏" : emojiKey;
                          const names = item.reactedBy?.[emojiKey];
                          const label = names?.length ? (names.length <= 3 ? names.join(", ") : `${names.slice(0, 3).join(", ")} +${names.length - 3} more`) : "";
                          return (
                            <button key={emojiKey} type="button" className={`reaction-btn ${item.reacted?.[emojiKey] ? "reaction-btn--active" : ""}`} onClick={() => void toggleLocalFeedReaction(item.id, emojiKey)}>
                              {display} {count}
                              {label ? <span className="reaction-tooltip">{label}</span> : null}
                            </button>
                          );
                        });
                      })()}
                      <button type="button" className="reaction-btn reaction-btn--add" onClick={() => setEmojiPickerPostId(emojiPickerPostId === item.id ? null : item.id)} title="Add reaction">+</button>
                      {emojiPickerPostId === item.id ? (
                        <div className="emoji-picker">
                          {["🔥", "🧠", "👏", "⭐", "🎯", "💪", "📚", "⚡", "🎉", "🏆", "✨", "💡", "🎓", "🚀", "💎", "🌟", "📖", "🕐", "💯", "🙌", "🤯", "😤", "👑", "🌊", "😂", "🤣", "😭", "🥲", "😅", "🥹", "❤️", "🙏", "👍", "👎", "😍", "😎"].map((emj) => (
                            <button key={emj} type="button" className={`emoji-picker__item ${item.reacted?.[emj] ? "emoji-picker__item--active" : ""}`} onClick={() => { void toggleLocalFeedReaction(item.id, emj); setEmojiPickerPostId(null); }}>{emj}</button>
                          ))}
                        </div>
                      ) : null}
                    </div>
                    <div className="feed-card__comments-shell">
                      <button type="button" className="feed-comments-toggle" onClick={() => toggleFeedComments(item.id)} aria-expanded={commentsOpen}>
                        {appStyle === "wabi-sabi" ? `Reply${comments.length ? ` · ${comments.length}` : ""}` : `Comments (${comments.length}) ${commentsOpen ? "↑" : "↓"}`}
                      </button>
                      {commentsOpen ? (
                        <div className="feed-card__comments">
                          {comments.length ? comments.map((comment: SocialFeedComment) => {
                            const commentTarget = { userId: comment.userId, displayName: comment.displayName, friendCode: comment.friendCode, avatar: comment.isSelf ? state.social.avatar : comment.avatar };
                            return (
                              <div key={comment.id} className="feed-comment">
                                <ArenaAvatar name={comment.displayName} avatar={comment.isSelf ? state.social.avatar : comment.avatar} self={comment.isSelf} />
                                <div>
                                  <div className="feed-comment__meta">
                                    <button type="button" className="social-name-button social-name-button--strong" onClick={() => void openFriendProfile(commentTarget)}>{comment.displayName}{comment.isSelf ? " (You)" : ""}</button>
                                    <span>{formatFeedPostedAt(comment.createdAt)}</span>
                                  </div>
                                  <p>{comment.body}</p>
                                </div>
                              </div>
                            );
                          }) : <p className="feed-comments-empty">No comments yet. Start the reply chain.</p>}
                          <form className="feed-comment-form" onSubmit={(event) => void submitFeedComment(event, item)}>
                            <input
                              className="arena-input"
                              value={feedCommentDrafts[item.id] ?? ""}
                              onChange={(event) => setFeedCommentDrafts((current) => ({ ...current, [item.id]: event.target.value }))}
                              maxLength={220}
                              placeholder="Reply or comment under this post..."
                              disabled={feedCommentSavingId === item.id}
                            />
                            <button type="submit" className="arena-btn arena-btn--send" disabled={feedCommentSavingId === item.id || !(feedCommentDrafts[item.id] ?? "").trim()}>{feedCommentSavingId === item.id ? "Posting" : "Reply"}</button>
                          </form>
                        </div>
                      ) : null}
                    </div>
                  </article>
                );
              }) : (
                <div className="arena-empty"><strong>No feed posts yet</strong><span>Post a session or sync to pull the latest arena activity.</span></div>
              )}

              {appStyle !== "wabi-sabi" ? <><div className="section-label">This Week</div>
              <article className="week-compare">
                {weekCompareEntries.length ? weekCompareEntries.map((entry) => {
                  const max = Math.max(1, ...weekCompareEntries.map((item) => item.minutes));
                  return (
                    <div key={entry.userId} className="week-compare__row">
                      <ArenaAvatar name={entry.displayName} avatar={entry.isSelf ? state.social.avatar : entry.avatar} self={entry.isSelf} />
                      <strong>{entry.displayName}{entry.isSelf ? " (You)" : ""}</strong>
                      <div className="week-compare__bar-wrap"><span style={{ width: `${(entry.minutes / max) * 100}%` }} /></div>
                      <small>{formatMinutes(entry.minutes)}</small>
                    </div>
                  );
                }) : <p className="empty-copy">Weekly comparison appears after you sync with friends.</p>}
              </article></> : <p className="wabi-feed-end">That is the whole feed. No counts, no ranks, nothing new until someone shares again.</p>}
              </div>
            </div>
          ) : null}

          {socialSubtab === "profile" ? (
            <div className={`social-single-panel ${appStyle === "wabi-sabi" ? "wabi-profile-panel" : ""}`}>
              <article className="arena-player-card">
                <div className="arena-player-card__inner">
                  <div className="arena-player-main">
                    <button type="button" className="profile-avatar-button" onClick={openProfileAvatarEditor} title="Change profile avatar">
                      <ArenaAvatar name={state.social.displayName} avatar={state.social.avatar} self size="lg" />
                      <span>Edit</span>
                    </button>
                    <div className="arena-player-copy">
                      {socialNameEditing ? (
                        <form className="arena-name-edit" onSubmit={saveSocialName}>
                          <input className="arena-name-input" value={socialNameDraft} onChange={(event) => setSocialNameDraft(event.target.value)} maxLength={48} autoFocus />
                          <button type="submit" className="arena-icon-button arena-icon-button--save" title="Save player name">✓</button>
                          <button type="button" className="arena-icon-button" onClick={cancelEditingSocialName} title="Cancel">×</button>
                        </form>
                      ) : (
                        <div className="arena-name-row">
                          <h2>{state.social.displayName}</h2>
                          <button type="button" className="arena-name-edit-button" onClick={startEditingSocialName}>Edit</button>
                        </div>
                      )}
                      <div className="arena-player-tags">
                        <button type="button" className="arena-code-plate" onClick={copyFriendCode} title="Copy player tag">
                          <span className="arena-code-key">#</span>
                          <span>{state.social.friendCode}</span>
                          <span className="arena-code-copy">⧉</span>
                        </button>
                        <button type="button" className="arena-code-plate" onClick={copyFriendInviteLink} title="Copy invite link">
                          <span className="arena-code-key">↗</span>
                          <span>Invite link</span>
                          <span className="arena-code-copy">⧉</span>
                        </button>
                        <button type="button" className="arena-code-plate profile-badges-button" onClick={() => setBadgesOpen(true)} title="View badges">
                          <span className="arena-code-key">★</span>
                          <span>Badges</span>
                          <span className="arena-code-copy">↗</span>
                        </button>
                        <span className={`arena-sync-pill ${state.social.lastSyncError ? "arena-sync-pill--error" : socialConfigured ? "arena-sync-pill--ready" : "arena-sync-pill--local"}`}>
                          <span />{state.social.lastSyncError ? "Sync Issue" : socialConfigured ? "Arena Synced" : "Local Only"}
                        </span>
                      </div>
                    </div>
                  </div>

                  <div className="arena-player-stats">
                    <div className="arena-mini-stat arena-mini-stat--daily"><span className="arena-mini-stat__icon">↯</span><div><strong>{formatMinutes(localSocialDaily.minutes)}</strong><span>Today · {localSocialDaily.sessions} ses.</span></div></div>
                    <div className="arena-mini-stat arena-mini-stat--weekly"><span className="arena-mini-stat__icon">◆</span><div><strong>{formatMinutes(localSocialWeekly.minutes)}</strong><span>This Week · {localSocialWeekly.sessions} ses.</span></div></div>
                    <div className="arena-mini-stat arena-mini-stat--overall"><span className="arena-mini-stat__icon">★</span><div><strong>{formatMinutes(localSocialOverall.minutes)}</strong><span>All Time · {localSocialOverall.sessions} ses.</span></div></div>
                    <div className="arena-mini-stat"><span className="arena-mini-stat__icon">📅</span><div><strong>{formatMinutes(localSocialMonthly.minutes)}</strong><span>This Month · {localSocialMonthly.sessions} ses.</span></div></div>
                    <div className="arena-mini-stat"><span className="arena-mini-stat__icon">⚔</span><div><strong>{state.social.isPrivate ? "Hidden" : myGlobalRank ? `#${myGlobalRank}` : "—"}</strong><span>Global Rank</span></div></div>
                    <div className="arena-mini-stat"><span className="arena-mini-stat__icon">👥</span><div><strong>{myFriendRank ? `#${myFriendRank}` : "—"}</strong><span>Friends Rank</span></div></div>
                  </div>
                </div>

                <div className="arena-sync-console">
                  <div>
                    <span className="arena-kicker">Cloud sync</span>
                    <p>Last synced: {lastSocialSyncLabel}</p>
                    {state.social.lastSyncError ? <p className="arena-error">{state.social.lastSyncError}</p> : null}
                    {!socialConfigured ? <p className="arena-error">Cloudflare Worker URL has not been configured yet.</p> : null}
                  </div>
                  <button type="button" className="arena-btn arena-btn--send" onClick={() => void runSocialSync()} disabled={socialSyncing || !socialConfigured}>
                    {socialSyncing ? "Syncing..." : "Sync Arena"}
                  </button>
                </div>

                <div className="profile-options">
                  <button type="button" className={`profile-toggle ${state.social.isPrivate ? "" : "active"}`} onClick={toggleProfilePrivacy}>
                    <strong>{state.social.isPrivate ? "Private profile" : "Public profile"}</strong>
                    <span>{state.social.isPrivate ? "Hidden from global feed and leaderboard." : "Shown on global feed and leaderboard."}</span>
                  </button>
                    <button type="button" className={`profile-toggle ${state.social.autoPostSessions ? "active" : ""}`} onClick={toggleAutoPostSessions}>
                      <strong>{state.social.autoPostSessions ? "Auto-post on" : "Auto-post off"}</strong>
                      <span>{state.social.autoPostSessions ? "Completed sessions queue feed posts automatically." : "You choose which sessions to post."}</span>
                    </button>
                    <button type="button" className={`profile-toggle ${state.social.showHoursToFriends ? "active" : ""}`} onClick={toggleShowHoursToFriends}>
                      <strong>Show hours to friends</strong>
                      <span>Your totals in the friends standings.</span>
                    </button>
                </div>
              </article>
            </div>
          ) : null}

          {socialSubtab === "squad" ? (
            <div className={`social-single-panel squad-panel ${appStyle === "wabi-sabi" ? "wabi-squad-panel" : ""}`}>
              {!currentSquad ? (
                <>
                  <article className="arena-panel squad-card">
                    <div className="arena-panel-head">
                      <span className="arena-panel-icon">S</span>
                      <div>
                        <span className="arena-kicker">Create squad</span>
                        <h3>Start a squad</h3>
                      </div>
                    </div>
                    <p className="squad-copy">Squads hold up to 4 players. Once you join one, you cannot create or join another until you leave.</p>
                    <form className="squad-form" onSubmit={submitSquadCreate}>
                      <input className="arena-input" value={squadNameDraft} onChange={(event) => setSquadNameDraft(event.target.value)} placeholder="Squad name" maxLength={48} disabled={!socialConfigured || socialSyncing} />
                      <button type="button" className={`profile-toggle squad-privacy-toggle ${squadPrivateDraft ? "" : "active"}`} onClick={() => setSquadPrivateDraft((value) => !value)} disabled={!socialConfigured || socialSyncing}>
                        <strong>{squadPrivateDraft ? "Private squad" : "Public squad"}</strong>
                        <span>{squadPrivateDraft ? "Players must request to join." : "Players can join instantly."}</span>
                      </button>
                      <button type="submit" className="arena-btn arena-btn--send" disabled={!socialConfigured || socialSyncing}>{socialSyncing ? "Working..." : "Create"}</button>
                    </form>
                  </article>

                  <article className="arena-panel squad-card">
                    <div className="arena-panel-head">
                      <span className="arena-panel-icon">⌕</span>
                      <div>
                        <span className="arena-kicker">Search all squads</span>
                        <h3>Find a squad</h3>
                      </div>
                    </div>
                    <form className="squad-form squad-search-form" onSubmit={submitSquadSearch}>
                      <input className="arena-input" value={squadSearchDraft} onChange={(event) => setSquadSearchDraft(event.target.value)} placeholder="Search by squad name" disabled={!socialConfigured || squadSearching} />
                      <button type="submit" className="arena-btn arena-btn--send" disabled={!socialConfigured || squadSearching}>{squadSearching ? "Searching..." : "Search"}</button>
                    </form>
                    <div className="squad-suggestion-head">
                      <div>
                        <strong>Suggested squads</strong>
                        <span>Don't know a name? Join or request one of these.</span>
                      </div>
                      {squadSuggestionPool.length > 4 ? (
                        <button type="button" className="arena-btn arena-btn--decline" onClick={() => void loadSquadSuggestions()} disabled={squadSuggestionsLoading}>{squadSuggestionsLoading ? "Loading..." : "Reload"}</button>
                      ) : null}
                    </div>
                    <div className="squad-search-results squad-suggestions">
                      {squadSuggestions.map((squad) => (
                        <div key={squad.id} className="squad-search-card">
                          <div>
                            <strong>{squad.name}</strong>
                            <span>{squad.isPrivate ? "Private" : "Public"} · {squad.memberCount}/{squad.maxMembers} members · {formatMinutes(squad.totalMinutes)}</span>
                          </div>
                          {squad.action === "join" || squad.action === "request" ? (
                            <button type="button" className="arena-btn arena-btn--accept" onClick={() => void joinOrRequestSquad(squad)} disabled={socialSyncing}>{squad.action === "join" ? "Join" : "Request"}</button>
                          ) : (
                            <span className="arena-pending-badge">{squad.action === "pending" ? "Pending" : squad.action === "full" ? "Full" : "Unavailable"}</span>
                          )}
                        </div>
                      ))}
                      {squadSuggestionsLoading ? <div className="arena-empty small">Loading suggestions...</div> : null}
                      {!squadSuggestionsLoading && !squadSuggestions.length ? <div className="arena-empty small">No squads exist yet. Create the first one.</div> : null}
                    </div>
                    {state.social.outgoingSquadRequests.length ? (
                      <div className="squad-request-note">Pending request: {state.social.outgoingSquadRequests.map((request) => request.squadName ?? "Squad").join(", ")}</div>
                    ) : null}
                    <div className="squad-result-head">Search results</div>
                    <div className="squad-search-results">
                      {squadSearchResults.map((squad) => (
                        <div key={squad.id} className="squad-search-card">
                          <div>
                            <strong>{squad.name}</strong>
                            <span>{squad.isPrivate ? "Private" : "Public"} · {squad.memberCount}/{squad.maxMembers} members · {formatMinutes(squad.totalMinutes)}</span>
                          </div>
                          {squad.action === "join" || squad.action === "request" ? (
                            <button type="button" className="arena-btn arena-btn--accept" onClick={() => void joinOrRequestSquad(squad)} disabled={socialSyncing}>{squad.action === "join" ? "Join" : "Request"}</button>
                          ) : (
                            <span className="arena-pending-badge">{squad.action === "pending" ? "Pending" : squad.action === "full" ? "Full" : "Unavailable"}</span>
                          )}
                        </div>
                      ))}
                      {!squadSearchResults.length ? <div className="arena-empty small">Search by name to discover public and private squads.</div> : null}
                    </div>
                  </article>
                </>
              ) : (
                <>
                  <article className="arena-panel squad-card squad-hq-card">
                    <div className="squad-hq-head">
                      <div className="arena-title-cluster">
                        <span className="arena-title-icon">S</span>
                        <div>
                          <span className="arena-kicker">{currentSquad.isPrivate ? "Private squad" : "Public squad"}</span>
                          <h2>{currentSquad.name}</h2>
                          <p>{currentSquad.memberCount}/4 members · Your rank: {squadRoleLabels[currentSquad.myRole]}</p>
                        </div>
                      </div>
                      <div className="squad-actions">
                        {currentSquad.myRole === "leader" ? <button type="button" className="arena-btn arena-btn--decline" onClick={startSquadSettingsEdit}>Edit</button> : null}
                        <button type="button" className="arena-btn arena-btn--decline" onClick={() => void leaveCurrentSquad()} disabled={socialSyncing}>Leave</button>
                      </div>
                    </div>
                    {squadSettingsEditing ? (
                      <form className="squad-form squad-settings-form" onSubmit={submitSquadSettings}>
                        <input className="arena-input" value={squadSettingsNameDraft} onChange={(event) => setSquadSettingsNameDraft(event.target.value)} maxLength={48} />
                        <button type="button" className={`profile-toggle squad-privacy-toggle ${squadSettingsPrivateDraft ? "" : "active"}`} onClick={() => setSquadSettingsPrivateDraft((value) => !value)} disabled={socialSyncing}>
                          <strong>{squadSettingsPrivateDraft ? "Private squad" : "Public squad"}</strong>
                          <span>{squadSettingsPrivateDraft ? "Players must request to join." : "Players can join instantly."}</span>
                        </button>
                        <button type="submit" className="arena-btn arena-btn--send" disabled={socialSyncing}>Save</button>
                        <button type="button" className="arena-btn arena-btn--decline" onClick={() => setSquadSettingsEditing(false)}>Cancel</button>
                      </form>
                    ) : null}
                    <div className="squad-stats-grid">
                      <div><strong>{formatMinutes(currentSquad.totalMinutes)}</strong><span>Total squad focus</span></div>
                      <div><strong>{currentSquad.totalSessions}</strong><span>Sessions</span></div>
                      <div><strong>{state.social.incomingSquadRequests.length}</strong><span>Pending requests</span></div>
                    </div>
                  </article>

                  <div className="squad-main-grid">
                    <article className="arena-panel squad-card squad-roster-panel">
                      <div className="arena-panel-head"><span className="arena-panel-icon">R</span><div><span className="arena-kicker">Squad roster</span><h3>Members</h3></div></div>
                      <div className="squad-member-list">
                        {currentSquad.members.map((member) => {
                          const expanded = expandedSquadMemberId === member.userId;
                          const assignableRoles = currentSquadRole && !member.isSelf ? getAssignableSquadRoles(currentSquadRole, member.role) : [];
                          const canKickMember = Boolean(currentSquadRole && !member.isSelf && canKickSquadMember(currentSquadRole, member.role));
                          const canManageMember = assignableRoles.length > 0 || canKickMember;
                          return (
                            <div key={member.userId} className={`squad-member-card ${expanded ? "squad-member-card--expanded" : ""}`}>
                              <button type="button" className="squad-member-summary" onClick={() => setExpandedSquadMemberId((current) => current === member.userId ? null : member.userId)}>
                                <ArenaAvatar name={member.displayName} avatar={member.isSelf ? state.social.avatar : member.avatar} self={member.isSelf} size="sm" />
                                <div className="squad-member-main"><strong>{member.displayName}{member.isSelf ? " (You)" : ""}</strong><span>{member.friendCode} · {formatMinutes(member.minutes)} · {member.sessions} sessions</span></div>
                                <span className={`squad-role-badge squad-role-badge--${member.role}`}>{squadRoleLabels[member.role]}</span>
                              </button>
                              {expanded ? (
                                <div className="squad-member-expanded">
                                  <div>
                                    <strong>{canManageMember ? `Manage ${member.displayName}` : member.isSelf ? "This is you" : "No actions available"}</strong>
                                    <span>{squadRoleLabels[member.role]} · joined {formatProfileSeenAt(member.joinedAt)}</span>
                                  </div>
                                  {assignableRoles.length ? (
                                    <div className="squad-member-role-actions">
                                      {assignableRoles.map((role) => (
                                        <button key={role} type="button" className={`arena-btn ${member.role === role ? "arena-btn--send" : "arena-btn--decline"}`} onClick={() => void changeSquadMemberRole(member.userId, role)} disabled={socialSyncing || member.role === role}>
                                          Make {squadRoleLabels[role]}
                                        </button>
                                      ))}
                                    </div>
                                  ) : null}
                                  {canKickMember ? <button type="button" className="arena-btn arena-btn--decline squad-kick-expanded" onClick={() => void kickFromSquad(member.userId, member.displayName)} disabled={socialSyncing}>Kick from squad</button> : null}
                                </div>
                              ) : null}
                            </div>
                          );
                        })}
                      </div>
                    </article>

                    <article className="arena-panel squad-card squad-internal-leaderboard-panel">
                      <div className="arena-panel-head"><span className="arena-panel-icon">⚔</span><div><span className="arena-kicker">Internal leaderboard</span><h3>Squad members</h3></div></div>
                      <div className="arena-period-chips squad-periods" aria-label="Internal squad leaderboard period">
                        {(["daily", "weekly", "overall"] as SocialLeaderboardPeriod[]).map((period) => <button key={period} type="button" className={socialPeriod === period ? "arena-period-chip arena-period-chip--active" : "arena-period-chip"} onClick={() => setSocialPeriod(period)}>{period === "daily" ? "Daily" : period === "weekly" ? "Weekly" : "Overall"}</button>)}
                      </div>
                      <div className="arena-lb-rows squad-lb-rows">
                        {squadMemberLeaderboard.map((entry) => {
                          const profileTarget = { userId: entry.userId, displayName: entry.displayName, friendCode: entry.friendCode, avatar: entry.isSelf ? state.social.avatar : entry.avatar };
                          return <ArenaLeaderboardRow key={entry.userId} entry={entry} selfAvatar={state.social.avatar} onProfile={() => void openFriendProfile(profileTarget)} />;
                        })}
                        {!squadMemberLeaderboard.length ? <div className="arena-empty small">Sync your squad to see member rankings.</div> : null}
                      </div>
                    </article>
                  </div>

                  <article className="arena-panel squad-card squad-chat-card">
                    <div className="arena-panel-head"><span className="arena-panel-icon">#</span><div><span className="arena-kicker">Squad chat</span><h3>Chat</h3></div></div>
                    <div className="squad-chat-list">
                      {state.social.squadMessages.map((message) => (
                        <div key={message.id} className={`squad-chat-message ${message.isSelf ? "squad-chat-message--self" : ""}`}>
                          <ArenaAvatar name={message.displayName} avatar={message.isSelf ? state.social.avatar : message.avatar} self={message.isSelf} size="sm" />
                          <div><strong>{message.displayName} <span>{squadRoleLabels[message.role]}</span></strong><p>{message.body}</p></div>
                          {message.isSelf ? <button type="button" className="squad-chat-delete" onClick={() => void deleteOwnSquadMessage(message.id)} title="Delete message">Delete</button> : null}
                        </div>
                      ))}
                      {!state.social.squadMessages.length ? <div className="arena-empty small">No messages yet. Start the squad chat.</div> : null}
                    </div>
                    <form className="squad-chat-form" onSubmit={submitSquadChat}>
                      <input className="arena-input" value={squadChatDraft} onChange={(event) => setSquadChatDraft(event.target.value)} placeholder="Message your squad" maxLength={500} />
                      <button type="submit" className="arena-btn arena-btn--send" disabled={!squadChatDraft.trim()}>Send</button>
                    </form>
                  </article>

                  {canManageCurrentSquadRequests ? (
                    <article className="arena-panel squad-card squad-requests-card">
                      <div className="arena-panel-head"><span className="arena-panel-icon">?</span><div><span className="arena-kicker">Private requests</span><h3>Join requests</h3></div></div>
                      {state.social.incomingSquadRequests.length ? state.social.incomingSquadRequests.map((request) => (
                        <div key={request.id} className="arena-request-card">
                          <ArenaAvatar name={request.displayName ?? "Student"} avatar={request.avatar} size="sm" />
                          <div className="arena-request-copy"><strong>{request.displayName}</strong><span>{request.friendCode}</span></div>
                          <div className="arena-request-actions">
                            <button type="button" className="arena-btn arena-btn--accept" onClick={() => void answerSquadRequest(request.id, "accepted")} disabled={socialSyncing}>Accept</button>
                            <button type="button" className="arena-btn arena-btn--decline" onClick={() => void answerSquadRequest(request.id, "declined")} disabled={socialSyncing}>Decline</button>
                          </div>
                        </div>
                      )) : <div className="arena-empty small">No pending squad requests.</div>}
                    </article>
                  ) : null}
                </>
              )}
            </div>
          ) : null}

          {socialSubtab === "friends" ? (
            <div className={`social-single-panel ${appStyle === "wabi-sabi" ? "wabi-friends-panel" : ""}`}>
              <article className="arena-panel arena-friends-panel">
                <div className="arena-panel-head">
                  <span className="arena-panel-icon">+</span>
                  <div>
                    <span className="arena-kicker">Player tags</span>
                    <h3>Friends</h3>
                  </div>
                  </div>

                  <div className="arena-invite-card">
                    <div>
                      <span className="arena-kicker">Invite link</span>
                      <p>Share this link in WhatsApp or anywhere else. It opens the download page with your player tag ready to copy.</p>
                    </div>
                    <input className="arena-input" value={friendInviteLink} readOnly onFocus={(event) => event.currentTarget.select()} />
                    <button type="button" className="arena-btn arena-btn--send" onClick={copyFriendInviteLink}>Copy invite link</button>
                  </div>

                  <form className="arena-add-friend" onSubmit={submitFriendRequest}>
                    <input className="arena-input" value={friendCodeDraft} onChange={(event) => setFriendCodeDraft(event.target.value.toUpperCase())} placeholder="Enter player tag, e.g. ABCD-1234" disabled={!socialConfigured} />
                    <button type="submit" className="arena-btn arena-btn--send" disabled={socialSyncing || !socialConfigured}>Send</button>
                  </form>

                <div className="arena-social-section">
                  <h4>Incoming <span>{state.social.incomingFriendRequests.length}</span></h4>
                  {state.social.incomingFriendRequests.length ? state.social.incomingFriendRequests.map((request) => (
                    <div key={request.id} className="arena-request-card">
                      <ArenaAvatar name={request.fromDisplayName} avatar={request.fromAvatar} size="sm" />
                      <div className="arena-request-copy"><strong>{request.fromDisplayName}</strong><span>{request.fromFriendCode}</span></div>
                      <div className="arena-request-actions">
                        <button type="button" className="arena-btn arena-btn--accept" onClick={() => void answerFriendRequest(request.id, "accepted")} disabled={socialSyncing}>Accept</button>
                        <button type="button" className="arena-btn arena-btn--decline" onClick={() => void answerFriendRequest(request.id, "declined")} disabled={socialSyncing}>Decline</button>
                      </div>
                    </div>
                  )) : <div className="arena-empty small">No incoming requests.</div>}
                </div>

                <div className="arena-social-section">
                  <h4>Pending <span>{state.social.outgoingFriendRequests.length}</span></h4>
                  {state.social.outgoingFriendRequests.length ? state.social.outgoingFriendRequests.map((request) => (
                    <div key={request.id} className="arena-request-card">
                      <ArenaAvatar name={request.toDisplayName} avatar={request.toAvatar} size="sm" />
                      <div className="arena-request-copy"><strong>{request.toDisplayName}</strong><span>{request.toFriendCode}</span></div>
                      <span className="arena-pending-badge">Pending</span>
                    </div>
                  )) : <div className="arena-empty small">No pending sent requests.</div>}
                </div>

                <div className="arena-social-section">
                  <h4>Your Friends <span>{state.social.friends.length}</span></h4>
                  {state.social.friends.length ? (
                    <div className="arena-friend-grid">
                      {state.social.friends.map((friend) => (
                        <div key={friend.userId} className="arena-friend-card" onClick={() => void openFriendProfile(friend)} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openFriendProfile(friend); } }}>
                          <ArenaAvatar name={friend.displayName} avatar={friend.avatar} size="sm" />
                          <div><strong>{friend.displayName}</strong><span>{friend.friendCode}{friend.lastSeenAt ? ` · seen ${formatProfileSeenAt(friend.lastSeenAt)}` : ""}</span></div>
                        </div>
                      ))}
                    </div>
                  ) : <div className="arena-empty">Share your player tag to build your friends leaderboard.</div>}
                </div>
              </article>
            </div>
          ) : null}

          {socialSubtab === "leaderboard" ? (
            <article className={`arena-leaderboard ${appStyle === "wabi-sabi" ? "wabi-standings" : ""}`}>
              <div className="arena-leaderboard-head">
                <div className="arena-title-cluster">
                  <span className="arena-title-icon">⚔</span>
                  <div>
                    <span className="arena-kicker">{appStyle === "wabi-sabi" ? "Standings" : "Arena standings"}</span>
                    <h2>{appStyle === "wabi-sabi" ? socialScope === "global" ? "World" : socialScope === "squad" ? "Squad" : "Friends" : socialArenaTitle}</h2>
                    <p>{socialArenaSubtitle}</p>
                  </div>
                </div>
                <div className="arena-sync-status">
                  <span>Last synced: {lastSocialSyncLabel}</span>
                  <button type="button" className="arena-btn arena-btn--decline social-refresh-btn" onClick={() => void runSocialSync()} disabled={socialSyncing || !socialConfigured}>
                    {socialSyncing ? "Syncing..." : "Refresh"}
                  </button>
                </div>
              </div>

              <div className="arena-scope-toggle" aria-label="Leaderboard scope">
                {(["friends", "squad", "global"] as SocialLeaderboardScope[]).map((scope) => (
                  <button key={scope} type="button" className={socialScope === scope ? "arena-scope-btn arena-scope-btn--active" : "arena-scope-btn"} onClick={() => setSocialScope(scope)}>
                    {scope === "global" ? "World Arena" : scope === "squad" ? "Squad Arena" : "Friends Arena"}
                  </button>
                ))}
              </div>

              {socialScope === "squad" ? (
                <div className="arena-period-chips" aria-label="Squad leaderboard period">
                  {(["daily", "season", "overall"] as SocialSquadScorePeriod[]).map((period) => (
                    <button key={period} type="button" className={squadScorePeriod === period ? "arena-period-chip arena-period-chip--active" : "arena-period-chip"} onClick={() => setSquadScorePeriod(period)}>
                      {period === "daily" ? "Daily" : period === "season" ? "Seasonal Points" : "Overall Points"}
                    </button>
                  ))}
                </div>
              ) : (
                <div className="arena-period-chips" aria-label="Leaderboard period">
                  {(["daily", "weekly", "overall"] as SocialLeaderboardPeriod[]).map((period) => (
                    <button key={period} type="button" className={socialPeriod === period ? "arena-period-chip arena-period-chip--active" : "arena-period-chip"} onClick={() => setSocialPeriod(period)}>
                      {period === "daily" ? "Daily Sprint" : period === "weekly" ? "Weekly League" : "Hall of Focus"}
                    </button>
                  ))}
                </div>
              )}

              {socialScope === "global" && state.social.isPrivate ? (
                <div className="arena-empty small arena-private-notice">
                  <strong>Private profile enabled</strong>
                  <span>You are hidden from the global leaderboard. Switch to Friends Arena to compare with friends.</span>
                </div>
              ) : null}

              {socialScope === "squad" ? (
                <div className="arena-empty small arena-private-notice">
                  <strong>{SQUAD_SEASON_NAME}</strong>
                  <span>Season: {SQUAD_SEASON_RANGE_LABEL} · Overall tracking started: {SQUAD_TRACKING_START_LABEL}</span>
                </div>
              ) : null}

              {socialScope !== "squad" && appStyle === "field-notebook" ? (
                <div className="fn-lb-table">
                  <div className="fn-lb-head">
                    <span />
                    <span>Name</span>
                    <span>Hours</span>
                    <span>{socialPeriod === "daily" ? "Today" : socialPeriod === "overall" ? "All time" : "This week"}</span>
                    <span>Sessions</span>
                  </div>
                  <div className="arena-lb-rows">
                    {socialLeaderboard.map((entry) => {
                      const profileTarget = { userId: entry.userId, displayName: entry.displayName, friendCode: entry.friendCode, avatar: entry.isSelf ? state.social.avatar : entry.avatar };
                      const barPct = entry.minutes > 0 ? Math.max(4, Math.round((entry.minutes / socialLeaderboardTopMinutes) * 100)) : 0;
                      return (
                        <div key={entry.userId} className={`fn-lb-row ${entry.isSelf ? "fn-lb-row--self" : ""}`} onClick={() => void openFriendProfile(profileTarget)} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openFriendProfile(profileTarget); } }}>
                          <span className="fn-lb-rank">{entry.rank}</span>
                          <span className="fn-lb-name">{entry.displayName}{entry.isSelf ? <em>{entry.friendCode}</em> : null}</span>
                          <span className="fn-lb-hours">{formatMinutes(entry.minutes)}</span>
                          <span className="fn-lb-bar"><i style={{ width: `${barPct}%` }} /></span>
                          <span className="fn-lb-sessions">{entry.sessions}</span>
                        </div>
                      );
                    })}
                    {!socialLeaderboard.length ? (
                      <div className="arena-empty">
                        <strong>No contenders yet</strong>
                        <span>Start studying and sync to claim your rank.</span>
                      </div>
                    ) : null}
                  </div>
                  {socialLeaderboard.length ? <p className="fn-lb-note">Bars are relative to the top of the board.</p> : null}
                </div>
              ) : null}

              {socialScope !== "squad" && appStyle !== "field-notebook" && socialLeaderboard.length >= 3 ? (
                <div className="arena-podium">
                  {[socialLeaderboard[1], socialLeaderboard[0], socialLeaderboard[2]].map((entry, index) => {
                    const profileTarget = { userId: entry.userId, displayName: entry.displayName, friendCode: entry.friendCode, avatar: entry.isSelf ? state.social.avatar : entry.avatar };
                    return (
                      <div key={entry.userId} className={`arena-podium-col ${entry.isSelf ? "arena-podium-col--self" : ""}`} onClick={() => void openFriendProfile(profileTarget)} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openFriendProfile(profileTarget); } }}>
                        <ArenaAvatar name={entry.displayName} avatar={entry.isSelf ? state.social.avatar : entry.avatar} self={entry.isSelf} size={index === 1 ? "lg" : "md"} />
                        <strong>{entry.displayName}{entry.isSelf ? " (You)" : ""}</strong>
                        <span>{formatMinutes(entry.minutes)}</span>
                        <div className={`arena-podium-block arena-podium-block--${entry.rank}`}>
                          <ArenaRankBadge rank={entry.rank} large={index === 1} />
                          <small>{entry.sessions} sessions</small>
                        </div>
                      </div>
                    );
                  })}
                </div>
              ) : null}

              {socialScope === "squad" || appStyle !== "field-notebook" ? (
                <div className="arena-lb-rows">
                  {socialScope === "squad" ? squadScoreLeaderboard.map((entry) => (
                    <SquadArenaRow key={entry.squadId} entry={entry} period={squadScorePeriod} isSelf={entry.squadId === state.social.squad?.id} onOpen={() => void openSquadDetails(entry)} />
                  )) : (socialLeaderboard.length >= 3 ? socialLeaderboard.slice(3) : socialLeaderboard).map((entry) => {
                    const profileTarget = { userId: entry.userId, displayName: entry.displayName, friendCode: entry.friendCode, avatar: entry.isSelf ? state.social.avatar : entry.avatar };
                    return <ArenaLeaderboardRow key={entry.userId} entry={entry} selfAvatar={state.social.avatar} onProfile={() => void openFriendProfile(profileTarget)} />;
                  })}
                  {socialScope === "squad" && !squadScoreLeaderboard.length ? (
                    <div className="arena-empty">
                      <strong>No eligible squads yet</strong>
                      <span>Squads appear in the Squad Arena once they have at least 1 member.</span>
                    </div>
                  ) : null}
                  {socialScope !== "squad" && !socialLeaderboard.length ? (
                    <div className="arena-empty">
                      <strong>No contenders yet</strong>
                      <span>Start studying and sync to claim your rank.</span>
                    </div>
                  ) : null}
                </div>
              ) : null}
            </article>
          ) : null}
        </section>
      ) : null}

      {expandedFeedImage?.imageUrl ? (
        <div className="feed-image-lightbox" onClick={() => setExpandedFeedImageId(null)} role="presentation">
          <button type="button" className="feed-image-lightbox__frame" onClick={() => setExpandedFeedImageId(null)} aria-label="Close fullscreen feed image">
            <img src={`${expandedFeedImage.imageUrl}${expandedFeedImage.imageUrl.includes("?") ? "&" : "?"}v=${encodeURIComponent(expandedFeedImage.imageExpiresAt ?? expandedFeedImage.createdAt)}`} alt={`${expandedFeedImage.displayName}'s feed post image`} />
          </button>
        </div>
      ) : null}

      {viewingFriend ? (
        <div className="calendar-drawer-backdrop" style={{ justifyContent: "center", alignItems: "center" }} onClick={() => setViewingFriend(null)} role="presentation">
          <article className="arena-player-card" style={{ width: "min(440px, 100%)", padding: 0, alignSelf: "center" }} onClick={(e) => e.stopPropagation()} role="dialog" aria-label={`${viewingFriend.displayName}'s profile`}>
            <div className="arena-player-card__inner">
              <div className="arena-title-cluster" style={{ marginBottom: 16 }}>
                <ArenaAvatar name={viewingFriend.displayName} avatar={viewingFriend.avatar} size="lg" />
                <div style={{ flex: 1 }}>
                  <span className="arena-kicker">{viewingFriend.friendCode}</span>
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <h3 style={{ margin: 0 }}>{viewingFriend.displayName}</h3>
                    <button type="button" className="arena-icon-button" onClick={() => setViewingFriend(null)} title="Close" style={{ marginLeft: "auto" }}>×</button>
                  </div>
                  {viewingFriend.lastSeenAt ? <span style={{ color: "var(--muted)", fontSize: "0.82rem" }}>Seen {formatProfileSeenAt(viewingFriend.lastSeenAt)}</span> : null}
                  <div className="profile-action-row">
                    <span className="arena-pending-badge">{viewingIsSelf ? "Your profile" : viewingIsFriend ? "Friend" : viewingRequestPending ? "Request pending" : "Not friends"}</span>
                    {!viewingIsSelf && !viewingIsFriend && !viewingRequestPending ? (
                      <button type="button" className="arena-btn arena-btn--send" onClick={() => void sendFriendRequestToCode(viewingFriend.friendCode)} disabled={socialSyncing || !socialConfigured}>Send friend request</button>
                    ) : null}
                  </div>
                </div>
              </div>
              {viewingFriendLoading ? (
                <div className="arena-empty">Loading stats...</div>
              ) : viewingFriendStats?.hoursVisible && viewingFriendStats.daily && viewingFriendStats.weekly && viewingFriendStats.overall ? (
                <div className="arena-player-stats" style={{ marginTop: 0 }}>
                  <div className="arena-mini-stat"><span className="arena-mini-stat__icon">↯</span><div><strong>{formatMinutes(viewingFriendStats.daily.minutes)}</strong><span>Today · {viewingFriendStats.daily.sessions} ses.</span></div></div>
                  <div className="arena-mini-stat"><span className="arena-mini-stat__icon">◆</span><div><strong>{formatMinutes(viewingFriendStats.weekly.minutes)}</strong><span>This Week · {viewingFriendStats.weekly.sessions} ses.</span></div></div>
                  <div className="arena-mini-stat"><span className="arena-mini-stat__icon">★</span><div><strong>{formatMinutes(viewingFriendStats.overall.minutes)}</strong><span>All Time · {viewingFriendStats.overall.sessions} ses.</span></div></div>
                  <div className="arena-mini-stat"><span className="arena-mini-stat__icon">📅</span><div><strong>{viewingFriendStats.daily.lastActiveDate || "—"}</strong><span>Last Active</span></div></div>
                </div>
              ) : viewingFriendStats ? (
                <div className="arena-empty">This friend has chosen not to share their study hours.</div>
              ) : (
                <div className="arena-error">Could not load stats.</div>
              )}
            </div>
          </article>
        </div>
      ) : null}

      {viewingSquadEntry ? (
        <div className="calendar-drawer-backdrop" style={{ justifyContent: "center", alignItems: "center" }} onClick={() => { setViewingSquadEntry(null); setViewingSquadDetails(null); }} role="presentation">
          <article className="arena-player-card squad-details-modal" onClick={(e) => e.stopPropagation()} role="dialog" aria-label={`${viewingSquadEntry.squadName} squad details`}>
            <div className="arena-player-card__inner">
              <div className="arena-title-cluster squad-details-head">
                <ArenaRankBadge rank={viewingSquadEntry.rank} large />
                <div style={{ flex: 1 }}>
                  <span className="arena-kicker">{viewingSquadDetails?.isPrivate ?? viewingSquadEntry.isPrivate ? "Private squad" : "Public squad"}</span>
                  <div className="squad-details-title-row">
                    <h3>{viewingSquadDetails?.name ?? viewingSquadEntry.squadName}</h3>
                    <button type="button" className="arena-icon-button" onClick={() => { setViewingSquadEntry(null); setViewingSquadDetails(null); }} title="Close">×</button>
                  </div>
                  <div className="profile-action-row">
                    <span className="arena-pending-badge">
                      {viewingSquadAction === "current" ? "Your squad" : viewingSquadAction === "pending" ? "Request pending" : viewingSquadAction === "full" ? "Full" : viewingSquadAction === "unavailable" ? "Unavailable" : viewingSquadAction === "request" ? "Request to join" : "Open to join"}
                    </span>
                    {viewingSquadAction === "join" || viewingSquadAction === "request" ? (
                      <button type="button" className="arena-btn arena-btn--send" onClick={() => void joinOrRequestViewedSquad()} disabled={socialSyncing || !socialConfigured}>
                        {viewingSquadAction === "join" ? "Join squad" : "Request to join"}
                      </button>
                    ) : null}
                  </div>
                </div>
              </div>

              {viewingSquadLoading ? (
                <div className="arena-empty">Loading squad...</div>
              ) : viewingSquadDetails ? (
                <>
                  <div className="arena-player-stats squad-details-stats" style={{ marginTop: 0 }}>
                    <div className="arena-mini-stat"><span className="arena-mini-stat__icon">#</span><div><strong>{viewingSquadDetails.memberCount}/{viewingSquadDetails.maxMembers}</strong><span>Members</span></div></div>
                    <div className="arena-mini-stat"><span className="arena-mini-stat__icon">◆</span><div><strong>{formatMinutes(viewingSquadDetails.totalMinutes)}</strong><span>Today</span></div></div>
                    <div className="arena-mini-stat"><span className="arena-mini-stat__icon">★</span><div><strong>{viewingSquadEntry.points} pts</strong><span>{squadScorePeriod === "season" ? "Season" : squadScorePeriod === "overall" ? "Overall" : "Today if held"}</span></div></div>
                    <div className="arena-mini-stat"><span className="arena-mini-stat__icon">↯</span><div><strong>{formatMinutes(Math.round(viewingSquadDetails.previousDayAverageMinutes ?? 0))}</strong><span>Yesterday avg</span></div></div>
                  </div>

                  <div className="squad-details-roster">
                    <div className="section-label">Members</div>
                    {viewingSquadDetails.members.map((member) => (
                      <div key={member.userId} className="squad-details-member">
                        <ArenaAvatar name={member.displayName} avatar={member.isSelf ? state.social.avatar : member.avatar} self={member.isSelf} size="sm" />
                        <div>
                          <strong>{member.displayName}{member.isSelf ? " (You)" : ""}</strong>
                          <span>{squadRoleLabels[member.role]} · today {formatMinutes(member.minutes)} · {member.sessions} sessions</span>
                        </div>
                      </div>
                    ))}
                  </div>
                </>
              ) : (
                <div className="arena-error">Could not load squad details.</div>
              )}
            </div>
          </article>
        </div>
      ) : null}

      {badgesOpen ? (
        <div className="calendar-drawer-backdrop profile-badges-backdrop" onClick={() => setBadgesOpen(false)} role="presentation">
          <article className="profile-badges-modal" onClick={(event) => event.stopPropagation()} role="dialog" aria-label="Profile badges">
            <div className="profile-badges-head">
              <div>
                <span className="arena-kicker">Profile collection</span>
                <h3>Badges</h3>
                <p>Hover or focus a badge to see how to unlock it.</p>
              </div>
              <button type="button" className="arena-icon-button" onClick={() => setBadgesOpen(false)} title="Close">×</button>
            </div>

            <div className="profile-badges-groups">
              {profileBadgeGroups.map((group) => (
                <section key={group.category} className="profile-badge-group">
                  <div className="profile-badge-group-head">
                    <h4>{group.category}</h4>
                    <span>{group.source}</span>
                  </div>
                  <div className="profile-badge-grid">
                    {group.badges.map(renderProfileBadgeCard)}
                  </div>
                  {group.subgroups?.map((subgroup) => (
                    <div key={subgroup.category} className="profile-badge-subgroup">
                      <div className="profile-badge-subgroup-head">
                        <h5>{subgroup.category}</h5>
                        <span>{subgroup.source}</span>
                      </div>
                      <div className="profile-badge-grid">
                        {subgroup.badges.map(renderProfileBadgeCard)}
                      </div>
                    </div>
                  ))}
                </section>
              ))}
            </div>
          </article>
        </div>
      ) : null}

      {profileAvatarEditorOpen ? (
        <div className="calendar-drawer-backdrop" style={{ justifyContent: "center", alignItems: "center" }} onClick={closeProfileAvatarEditor} role="presentation">
          <article className="profile-avatar-editor" onClick={(event) => event.stopPropagation()} role="dialog" aria-label="Edit profile avatar">
            <button type="button" className="arena-icon-button profile-avatar-close" onClick={closeProfileAvatarEditor} title="Close">×</button>
            <div className="profile-avatar-editor-head">
              <ArenaAvatar name={state.social.displayName} avatar={profileAvatarDraft} self size="lg" />
              <div>
                <span className="arena-kicker">Profile picture</span>
                <h3>Choose your arena mark</h3>
                <p>Friends will see this after your next arena sync.</p>
              </div>
            </div>

            <div className="profile-avatar-mode-toggle" aria-label="Avatar type">
              <button type="button" className={profileAvatarDraft.kind === "letter" ? "active" : ""} onClick={() => setProfileAvatarDraft({ kind: "letter", letter: getFirstAvatarLetter(state.social.displayName), style: "classic" })}>Letter</button>
              <button type="button" className={profileAvatarDraft.kind === "icon" ? "active" : ""} onClick={() => setProfileAvatarDraft({ kind: "icon", icon: avatarIcons[0] })}>Icon</button>
              <button type="button" className={profileAvatarDraft.kind === "photo" ? "active" : ""} onClick={() => setProfileAvatarDraft(profileAvatarDraft.kind === "photo" ? profileAvatarDraft : { kind: "photo", name: "", url: "", mimeType: "image/webp" })}>Photo</button>
            </div>

            {profileAvatarDraft.kind === "letter" ? (
              <div className="profile-avatar-panel">
                <div className="profile-avatar-grid profile-avatar-grid--styles">
                  {avatarStyles.map((style) => {
                    const avatar: SocialAvatar = { kind: "letter", letter: profileAvatarDraft.letter, style: style.id };
                    const selected = profileAvatarDraft.kind === "letter" && profileAvatarDraft.style === style.id;
                    return (
                      <button key={style.id} type="button" className={`profile-avatar-choice ${selected ? "selected" : ""}`} onClick={() => setProfileAvatarDraft(avatar)}>
                        <ArenaAvatar name={state.social.displayName} avatar={avatar} self size="md" />
                        <span>{style.label}</span>
                      </button>
                    );
                  })}
                </div>
                <button type="button" className="ghost-button small-button profile-avatar-change-letter" onClick={() => setProfileAvatarLetterPickerOpen((open) => !open)}>
                  Change letter
                </button>
                {profileAvatarLetterPickerOpen ? (
                  <div className="profile-avatar-letter-picker" aria-label="Choose avatar letter">
                    {alphabetLetters.map((letter) => (
                      <button key={letter} type="button" className={profileAvatarDraft.letter === letter ? "selected" : ""} onClick={() => setProfileAvatarDraft({ ...profileAvatarDraft, letter })}>
                        {letter}
                      </button>
                    ))}
                  </div>
                ) : null}
              </div>
            ) : profileAvatarDraft.kind === "icon" ? (
              <div className="profile-avatar-grid profile-avatar-grid--icons">
                {avatarIcons.map((icon) => {
                  const avatar: SocialAvatar = { kind: "icon", icon };
                  const selected = profileAvatarDraft.kind === "icon" && profileAvatarDraft.icon === icon;
                  return (
                    <button key={icon} type="button" className={`profile-avatar-choice ${selected ? "selected" : ""}`} onClick={() => setProfileAvatarDraft(avatar)}>
                      <ArenaAvatar name={state.social.displayName} avatar={avatar} self size="md" />
                    </button>
                  );
                })}
              </div>
            ) : (
              <div className="profile-avatar-panel">
                <button type="button" className="profile-avatar-photo-upload" onClick={() => profileAvatarFileInputRef.current?.click()}>
                  {profileAvatarDraft.url ? "Change photo" : "Choose a photo"}
                </button>
                <input ref={profileAvatarFileInputRef} type="file" accept="image/*" hidden onChange={(event) => void handleProfileAvatarPhotoChange(event)} />
                {profileAvatarDraft.url ? (
                  <button type="button" className="ghost-button small-button profile-avatar-photo-remove" onClick={() => setProfileAvatarDraft({ kind: "photo", name: "", url: "", mimeType: "image/webp" })}>Remove photo</button>
                ) : null}
                <p className="profile-avatar-photo-hint">Square photos look best. Up to 512px, under 1 MB. Most photo formats work, including HEIC.</p>
              </div>
            )}

            <div className="profile-avatar-actions">
              <button type="button" className="ghost-button small-button" onClick={closeProfileAvatarEditor}>Cancel</button>
              <button type="button" className="arena-btn arena-btn--send" onClick={() => void saveProfileAvatar()} disabled={socialSyncing}>Save</button>
            </div>
          </article>
        </div>
      ) : null}

      {avatarCropOpen && avatarCropSource ? (
        <div className="calendar-drawer-backdrop" style={{ justifyContent: "center", alignItems: "center" }} onClick={cancelAvatarCrop} role="presentation">
          <article className="profile-crop-editor" onClick={(event) => event.stopPropagation()} role="dialog" aria-label="Crop profile picture">
            <div className="profile-crop-head">
              <div>
                <span className="arena-kicker">Profile picture</span>
                <h3>Crop your photo</h3>
                <p>Drag to reposition, scroll or use the slider to zoom. The result is a square picture.</p>
              </div>
            </div>
            <div
              className={`profile-crop-stage ${avatarCropDragging ? "profile-crop-stage--dragging" : ""}`}
              style={{ width: AVATAR_CROP_VIEWPORT_PX, height: AVATAR_CROP_VIEWPORT_PX }}
              onPointerDown={handleAvatarCropPointerDown}
              onPointerMove={handleAvatarCropPointerMove}
              onPointerUp={handleAvatarCropPointerEnd}
              onPointerCancel={handleAvatarCropPointerEnd}
              onWheel={handleAvatarCropWheel}
            >
              <img
                src={avatarCropSource.url}
                alt="Profile picture to crop"
                draggable={false}
                style={{
                  width: avatarCropSource.width * Math.max(AVATAR_CROP_VIEWPORT_PX / avatarCropSource.width, AVATAR_CROP_VIEWPORT_PX / avatarCropSource.height) * avatarCrop.zoom,
                  height: avatarCropSource.height * Math.max(AVATAR_CROP_VIEWPORT_PX / avatarCropSource.width, AVATAR_CROP_VIEWPORT_PX / avatarCropSource.height) * avatarCrop.zoom,
                  transform: `translate(${AVATAR_CROP_VIEWPORT_PX / 2 - avatarCrop.x * avatarCropSource.width * Math.max(AVATAR_CROP_VIEWPORT_PX / avatarCropSource.width, AVATAR_CROP_VIEWPORT_PX / avatarCropSource.height) * avatarCrop.zoom}px, ${AVATAR_CROP_VIEWPORT_PX / 2 - avatarCrop.y * avatarCropSource.height * Math.max(AVATAR_CROP_VIEWPORT_PX / avatarCropSource.width, AVATAR_CROP_VIEWPORT_PX / avatarCropSource.height) * avatarCrop.zoom}px)`,
                }}
              />
              <div className="profile-crop-grid" aria-hidden="true" />
            </div>
            <div className="profile-crop-zoom-row">
              <span aria-hidden="true">−</span>
              <input type="range" min={1} max={AVATAR_CROP_MAX_ZOOM} step={0.01} value={avatarCrop.zoom} onChange={(event) => handleAvatarCropZoomChange(Number(event.target.value))} aria-label="Zoom" />
              <span aria-hidden="true">+</span>
            </div>
            <div className="profile-avatar-actions">
              <button type="button" className="ghost-button small-button" onClick={cancelAvatarCrop}>Cancel</button>
              <button type="button" className="arena-btn arena-btn--send" onClick={() => void confirmAvatarCrop()}>Apply crop</button>
            </div>
          </article>
        </div>
      ) : null}
    </>
  );
}
