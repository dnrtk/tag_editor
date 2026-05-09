use eframe::egui::{self, Key};

use crate::metadata;
use crate::state::{AppState, NavDirection};

/// Translates the current frame's key presses into discrete actions and applies them.
pub fn handle_keyboard(state: &mut AppState, ctx: &egui::Context) {
    for action in collect_actions(ctx, state) {
        apply(state, action);
    }
}

pub fn parse_hotkey(s: &str) -> Option<Key> {
    let trimmed = s.trim();
    if trimmed.chars().count() != 1 {
        return None;
    }
    let c = trimmed.chars().next()?.to_ascii_uppercase();
    match c {
        '0' => Some(Key::Num0),
        '1' => Some(Key::Num1),
        '2' => Some(Key::Num2),
        '3' => Some(Key::Num3),
        '4' => Some(Key::Num4),
        '5' => Some(Key::Num5),
        '6' => Some(Key::Num6),
        '7' => Some(Key::Num7),
        '8' => Some(Key::Num8),
        '9' => Some(Key::Num9),
        'A'..='Z' => Key::from_name(&c.to_string()),
        _ => None,
    }
}

enum Action {
    SaveTags,
    CloseImage,
    DeleteImage,
    Navigate(NavDirection),
    ToggleLeftSidebar,
    ToggleRightSidebar,
    ToggleTag(String),
}

fn collect_actions(ctx: &egui::Context, state: &AppState) -> Vec<Action> {
    // Suppress single-key shortcuts whenever a text widget has captured the keyboard,
    // so typing in the tag input or filter box doesn't navigate / delete by accident.
    // Modifier-bearing shortcuts (Ctrl+S etc.) still fire — they don't conflict with
    // text editing and users expect Ctrl+S to work mid-edit.
    let text_focused = ctx.wants_keyboard_input();

    let mut actions = Vec::new();
    ctx.input(|i| {
        let mods = i.modifiers;

        if mods.ctrl && i.key_pressed(Key::S) {
            actions.push(Action::SaveTags);
        }
        if mods.ctrl && i.key_pressed(Key::F) {
            actions.push(Action::ToggleLeftSidebar);
        }
        if mods.ctrl && i.key_pressed(Key::T) {
            actions.push(Action::ToggleRightSidebar);
        }

        if text_focused {
            return;
        }

        if i.key_pressed(Key::Escape) {
            actions.push(Action::CloseImage);
        }
        if i.key_pressed(Key::Delete) {
            actions.push(Action::DeleteImage);
        }
        if i.key_pressed(Key::ArrowLeft) && !mods.ctrl {
            actions.push(Action::Navigate(NavDirection::Prev));
        }
        if i.key_pressed(Key::ArrowRight) && !mods.ctrl {
            actions.push(Action::Navigate(NavDirection::Next));
        }

        // Hotkey-defined tag toggles. Skip when modifiers are involved so they don't
        // collide with the shortcuts above.
        if !mods.ctrl && !mods.alt {
            for (key_str, tag) in &state.config.hotkey_tags {
                if let Some(key) = parse_hotkey(key_str) {
                    if i.key_pressed(key) {
                        actions.push(Action::ToggleTag(tag.clone()));
                    }
                }
            }
        }
    });
    actions
}

fn apply(state: &mut AppState, action: Action) {
    match action {
        Action::SaveTags => state.save_tags(),
        Action::CloseImage => state.close_current_image(),
        Action::DeleteImage => state.delete_current_image(),
        Action::Navigate(dir) => state.navigate(dir),
        Action::ToggleLeftSidebar => {
            state.config.show_left_sidebar = !state.config.show_left_sidebar;
            state.config.save();
        }
        Action::ToggleRightSidebar => {
            state.config.show_right_sidebar = !state.config.show_right_sidebar;
            state.config.save();
        }
        Action::ToggleTag(tag) => {
            state.modify_tags(|tags| {
                metadata::toggle_tag(tags, &tag);
            });
        }
    }
}
