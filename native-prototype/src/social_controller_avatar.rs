//! The profile avatar editor and the badges dialog (Stage 22b): production's
//! `openProfileAvatarEditor`, the Letter / Icon / Photo modes, `handleProfileAvatarPhotoChange`,
//! the crop editor (`confirmAvatarCrop`) and `saveProfileAvatar` (photo upload, then the avatar
//! is saved and synced).

use std::sync::Arc;

use study_tracker_core::social::avatar::crop::{self, Crop};
use study_tracker_core::social::avatar::{first_avatar_letter, Avatar, AvatarStyle, AVATAR_ICONS};
use study_tracker_core::timer::WallTimestamp;

use super::feed::PreparedImage;
use super::{Pending, SocialController, SyncContext};
use crate::net::http::{HttpResponse, NetError};
use crate::net::images::DecodedImage;
use crate::net::social_ext;
use crate::net_jobs::Outgoing;

/// The editor's working avatar (`profileAvatarDraft`).
#[derive(Debug, Clone, PartialEq)]
pub enum AvatarDraft {
    Letter {
        letter: String,
        style: AvatarStyle,
    },
    Icon {
        icon: String,
    },
    /// `remote`: the stored photo; `local`: a cropped photo not uploaded yet. Neither: "Choose a
    /// photo".
    Photo {
        name: String,
        remote: Option<String>,
        local: Option<PreparedImage>,
    },
}

impl AvatarDraft {
    fn from_avatar(a: &Avatar) -> Self {
        match a {
            Avatar::Letter { letter, style } => Self::Letter {
                letter: letter.clone(),
                style: *style,
            },
            Avatar::Icon { icon } => Self::Icon { icon: icon.clone() },
            Avatar::Photo { name, url, .. } => Self::Photo {
                name: name.clone(),
                remote: a.remote_photo_url().map(|_| url.clone()),
                local: None,
            },
        }
    }
}

/// A picked photo, decoded (bounded) for the crop stage.
#[derive(Debug, Clone, PartialEq)]
pub struct CropSource {
    pub image: DecodedImage,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CropEditor {
    pub source: Arc<CropSource>,
    pub crop: Crop,
    drag: Option<(f64, f64, Crop)>,
}

impl CropEditor {
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    fn wh(&self) -> (f64, f64) {
        (
            f64::from(self.source.image.width),
            f64::from(self.source.image.height),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AvatarPending {
    Upload,
}

#[derive(Debug, Default)]
pub struct AvatarState {
    pub open: bool,
    pub draft: Option<AvatarDraft>,
    pub letter_picker: bool,
    pub crop: Option<CropEditor>,
    /// A picked photo / the crop result is being prepared off the UI thread.
    pub preparing: bool,
    pub uploading: bool,
    pub badges_open: bool,
}

impl SocialController {
    /// `openProfileAvatarEditor`.
    pub fn open_avatar_editor(&mut self) {
        let Some(p) = self.profile() else { return };
        self.avatar.draft = Some(AvatarDraft::from_avatar(&p.avatar));
        self.avatar.letter_picker = false;
        self.avatar.open = true;
    }

    /// `closeProfileAvatarEditor` (also Cancel and the backdrop).
    pub fn close_avatar_editor(&mut self) {
        // production lets the editor close while an upload is on the wire; the upload still
        // saves the avatar when it lands
        self.avatar.draft = None;
        self.avatar.letter_picker = false;
        self.avatar.open = false;
        self.avatar.crop = None;
    }

    /// The Letter / Icon / Photo toggle.
    pub fn avatar_mode(&mut self, mode: u8) {
        let name = self
            .profile()
            .map(|p| p.display_name.clone())
            .unwrap_or_default();
        let current = self.avatar.draft.clone();
        self.avatar.draft = Some(match mode {
            0 => AvatarDraft::Letter {
                letter: first_avatar_letter(&name),
                style: AvatarStyle::Classic,
            },
            1 => AvatarDraft::Icon {
                icon: AVATAR_ICONS[0].to_string(),
            },
            _ => match current {
                Some(p @ AvatarDraft::Photo { .. }) => p,
                _ => AvatarDraft::Photo {
                    name: String::new(),
                    remote: None,
                    local: None,
                },
            },
        });
    }

    pub fn avatar_style(&mut self, style: AvatarStyle) {
        if let Some(AvatarDraft::Letter { style: s, .. }) = self.avatar.draft.as_mut() {
            *s = style;
        }
    }

    pub fn avatar_letter(&mut self, letter: &str) {
        if let Some(AvatarDraft::Letter { letter: l, .. }) = self.avatar.draft.as_mut() {
            if letter.len() == 1 && letter.as_bytes()[0].is_ascii_uppercase() {
                *l = letter.to_string();
            }
        }
    }

    pub fn toggle_letter_picker(&mut self) {
        self.avatar.letter_picker = !self.avatar.letter_picker;
    }

    pub fn avatar_icon(&mut self, icon: &str) {
        if AVATAR_ICONS.contains(&icon) {
            self.avatar.draft = Some(AvatarDraft::Icon {
                icon: icon.to_string(),
            });
        }
    }

    /// "Remove photo".
    pub fn avatar_remove_photo(&mut self) {
        self.avatar.draft = Some(AvatarDraft::Photo {
            name: String::new(),
            remote: None,
            local: None,
        });
    }

    /// The picked photo was decoded (or refused, with production's message).
    pub fn crop_source_ready(&mut self, result: Result<CropSource, String>) {
        self.avatar.preparing = false;
        match result {
            Ok(source) => {
                self.avatar.crop = Some(CropEditor {
                    source: Arc::new(source),
                    crop: Crop::default(),
                    drag: None,
                });
            }
            Err(message) => self.say(message),
        }
    }

    pub fn crop_pointer_down(&mut self, x: f64, y: f64) {
        if let Some(c) = self.avatar.crop.as_mut() {
            c.drag = Some((x, y, c.crop));
        }
    }

    pub fn crop_pointer_move(&mut self, x: f64, y: f64) {
        if let Some(c) = self.avatar.crop.as_mut() {
            if let Some((sx, sy, start)) = c.drag {
                let (w, h) = c.wh();
                c.crop = crop::dragged(start, x - sx, y - sy, w, h);
            }
        }
    }

    pub fn crop_pointer_up(&mut self) {
        if let Some(c) = self.avatar.crop.as_mut() {
            c.drag = None;
        }
    }

    pub fn crop_wheel(&mut self, zoom_in: bool) {
        if let Some(c) = self.avatar.crop.as_mut() {
            let (w, h) = c.wh();
            c.crop = crop::wheeled(c.crop, zoom_in, w, h);
        }
    }

    pub fn crop_zoom(&mut self, value: f64) {
        if let Some(c) = self.avatar.crop.as_mut() {
            let (w, h) = c.wh();
            c.crop = crop::zoomed(c.crop, value, w, h);
        }
    }

    /// `cancelAvatarCrop`.
    pub fn crop_cancel(&mut self) {
        self.avatar.crop = None;
    }

    /// The crop was rendered and encoded (`confirmAvatarCrop`): it becomes the draft photo.
    pub fn crop_done(&mut self, result: Result<(String, PreparedImage), String>) {
        self.avatar.preparing = false;
        match result {
            Ok((name, image)) => {
                self.avatar.draft = Some(AvatarDraft::Photo {
                    name,
                    remote: None,
                    local: Some(image),
                });
                self.avatar.crop = None;
            }
            Err(message) => self.say(message),
        }
    }

    /// `saveProfileAvatar`.
    pub fn save_avatar(&mut self, now: WallTimestamp, ctx: &SyncContext) -> Vec<Outgoing> {
        if self.avatar.uploading || self.syncing || !self.active() {
            return Vec::new();
        }
        let Some(draft) = self.avatar.draft.clone() else {
            return Vec::new();
        };
        let avatar = match draft {
            AvatarDraft::Photo {
                local: Some(image),
                name,
                ..
            } => {
                let Some(id) = self.identity().cloned() else {
                    return Vec::new();
                };
                return match social_ext::avatar_upload(&id, &image.upload, &name) {
                    Ok(req) => {
                        self.avatar.uploading = true;
                        vec![self.send(Pending::Profile(AvatarPending::Upload), req)]
                    }
                    Err(err) => {
                        self.say(err.user_message("Could not upload profile photo."));
                        Vec::new()
                    }
                };
            }
            AvatarDraft::Photo {
                remote: Some(url),
                name,
                ..
            } => {
                let mime = self
                    .profile()
                    .and_then(|p| match &p.avatar {
                        Avatar::Photo { mime_type, .. } => Some(mime_type.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "image/webp".into());
                Avatar::Photo {
                    name,
                    url,
                    mime_type: mime,
                }
            }
            AvatarDraft::Photo { .. } => {
                self.say("Choose a photo before saving.");
                return Vec::new();
            }
            AvatarDraft::Letter { letter, style } => Avatar::Letter { letter, style },
            AvatarDraft::Icon { icon } => Avatar::Icon { icon },
        };
        self.avatar_saved(avatar, now, ctx)
    }

    fn avatar_saved(
        &mut self,
        avatar: Avatar,
        now: WallTimestamp,
        ctx: &SyncContext,
    ) -> Vec<Outgoing> {
        if let Some(r) = self.record.as_mut() {
            r.profile.avatar = avatar;
        }
        self.persist();
        self.avatar.open = false;
        self.avatar.letter_picker = false;
        self.avatar.draft = None;
        self.say("Profile avatar saved and synced.");
        self.sync(true, now, ctx)
    }

    pub fn set_badges_open(&mut self, open: bool) {
        self.avatar.badges_open = open;
    }

    pub(super) fn avatar_cancelled(&mut self, _p: AvatarPending) {
        self.avatar.uploading = false;
    }

    pub(super) fn avatar_reply(
        &mut self,
        _p: AvatarPending,
        result: Result<HttpResponse, NetError>,
        now: WallTimestamp,
        ctx: &SyncContext,
    ) -> Vec<Outgoing> {
        self.avatar.uploading = false;
        let name = self
            .profile()
            .map(|p| p.display_name.clone())
            .unwrap_or_default();
        match result.and_then(|r| social_ext::parse_avatar(&r, &name)) {
            Ok(avatar) => self.avatar_saved(avatar, now, ctx),
            Err(err) => {
                // the editor stays open with the cropped photo; the old avatar is kept
                self.say(err.user_message("Could not upload profile photo."));
                Vec::new()
            }
        }
    }
}
