//! Dedicated mobile experience (iPhone / compact screens < 600pt).
//!
//! Preserves the complete desktop/tablet layout for screens >= 600pt (iPad / Mac / PC),
//! while providing an intuitive, single-handed touch experience on iPhone:
//! - Full-screen canvas for PDF reading
//! - Top compact header with Hamburger menu (☰), document title, and quick toggles
//! - Collapsible drawers/overlays for Menu, Tools, and Inspector
//! - Floating bottom reader bar (page prev/next, page indicator, zoom fit)

use egui::{Align, Align2, Color32, CornerRadius, Layout, Rect, Sense, Stroke, vec2};

use crate::canvas::Fit;
use crate::comments::CommentTool;
use crate::theme::{self, ThemePreference, Tokens};
use crate::{Dialog, Mode, PdfCraftApp, PropsTab, QuickTool, RightPanel, icons, panels, widgets};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobileSheet {
    Menu,
    Tools,
    Inspector,
}

/// A compact screen threshold dividing phones (iPhone portrait < 600pt) from tablets (iPad >= 744pt) and desktop.
pub fn is_mobile(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() < 600.0
}

/// Render the mobile-optimized interface.
pub fn show(app: &mut PdfCraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());

    // 1. Mobile Top Header
    egui::Panel::top("mobile_header")
        .exact_size(44.0)
        .frame(
            egui::Frame::NONE
                .fill(t.chrome)
                .stroke(Stroke::new(1.0, t.divider))
                .inner_margin(egui::Margin::symmetric(8, 0)),
        )
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                // Left: Hamburger menu toggle button
                let is_menu_open = app.mobile_sheet == Some(MobileSheet::Menu);
                if icons::button(ui, "panel-left", 32.0, is_menu_open, tl!("Menu")).clicked() {
                    app.mobile_sheet = if is_menu_open { None } else { Some(MobileSheet::Menu) };
                }

                // Center: Title of document or "PdfCraft"
                let title = if let Some(i) = app.active {
                    crate::canvas::tab_label(app, i).unwrap_or_else(|| tl!("Document").to_string())
                } else {
                    "PdfCraft".to_string()
                };

                let avail_width = (ui.available_width() - 80.0).max(80.0);
                ui.allocate_ui_with_layout(
                    vec2(avail_width, 32.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        let label = egui::Label::new(
                            egui::RichText::new(title)
                                .font(theme::semibold(14.0))
                                .color(t.text),
                        )
                        .truncate();
                        ui.add(label);
                    },
                );

                // Right actions
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if app.active.is_some() {
                        let is_inspector_open = app.mobile_sheet == Some(MobileSheet::Inspector);
                        if icons::button(ui, "bookmark", 32.0, is_inspector_open, tl!("Bookmarks")).clicked() {
                            if app.right.is_none() {
                                app.right = Some(RightPanel::Pages);
                            }
                            app.mobile_sheet = if is_inspector_open { None } else { Some(MobileSheet::Inspector) };
                        }

                        let is_tools_open = app.mobile_sheet == Some(MobileSheet::Tools);
                        if icons::button(ui, "wrench", 32.0, is_tools_open, tl!("All tools")).clicked() {
                            app.mobile_sheet = if is_tools_open { None } else { Some(MobileSheet::Tools) };
                        }
                    } else if icons::button(ui, "folder", 32.0, false, tl!("Open")).clicked() {
                        app.open_dialog();
                    }
                });
            });
        });

    // 2. Central Viewport (100% full screen document canvas or home)
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(t.pasteboard))
        .show(ui, |ui| match app.active {
            None if app.combine_showing() => crate::combine_ui::page(app, ui),
            None => crate::home::show(app, ui),
            Some(index) => match crate::compare_ui::side_by_side(app, index) {
                Some((old, new)) => crate::compare_ui::compare_area(app, old, new, ui),
                None => crate::canvas::document_area(app, index, ui),
            },
        });

    // 3. Floating Bottom Reader Bar (HUD)
    if app.mobile_sheet.is_none() && let Some(index) = app.active {
        show_floating_reader_bar(app, index, ui, &t);
    }

    // 4. Modal Overlays / Drawers
    if let Some(sheet) = app.mobile_sheet {
        show_modal_sheet(app, sheet, ui, &t);
    }
}

/// Floating bottom reader HUD for smooth page navigation on mobile.
fn show_floating_reader_bar(app: &mut PdfCraftApp, index: usize, ui: &mut egui::Ui, t: &Tokens) {
    let Some((_doc_index, id)) = app.active_ids() else { return };
    let Some(doc) = app.session.get(id) else { return };
    let page_count = doc.info.pages.len();
    if page_count == 0 {
        return;
    }

    let screen = ui.ctx().content_rect();
    let hud_width = (screen.width() - 32.0).clamp(240.0, 320.0);

    egui::Area::new(egui::Id::new("mobile_reader_hud"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -14.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::NONE
                .fill(t.card.gamma_multiply(0.96))
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(20))
                .shadow(egui::Shadow {
                    offset: [0, 4],
                    blur: 16,
                    spread: 0,
                    color: Color32::from_black_alpha(if t.dark() { 120 } else { 35 }),
                })
                .inner_margin(egui::Margin::symmetric(10, 5))
                .show(ui, |ui| {
                    ui.set_width(hud_width);
                    ui.horizontal_centered(|ui| {
                        let view = &mut app.views[index];

                        // Previous page
                        let can_prev = view.current > 0;
                        if ui.add_enabled_ui(can_prev, |ui| {
                            icons::button(ui, "chevron-left", 28.0, false, tl!("Previous page"))
                        }).inner.clicked() {
                            view.step_page(false);
                        }

                        // Page indicator
                        let current = (view.current + 1).min(page_count);
                        let indicator = format!("{} / {}", current, page_count);
                        ui.allocate_ui_with_layout(
                            vec2(ui.available_width() - 80.0, 26.0),
                            Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(indicator)
                                        .font(theme::medium(13.0))
                                        .color(t.text),
                                );
                            },
                        );

                        // Next page
                        let can_next = view.current + 1 < page_count;
                        if ui.add_enabled_ui(can_next, |ui| {
                            icons::button(ui, "chevron-right", 28.0, false, tl!("Next page"))
                        }).inner.clicked() {
                            view.step_page(true);
                        }

                        ui.add_space(2.0);
                        ui.painter().vline(
                            ui.cursor().left(),
                            ui.max_rect().y_range().shrink(4.0),
                            Stroke::new(1.0, t.divider),
                        );
                        ui.add_space(4.0);

                        // Fit toggle button
                        let is_width = view.fit == Fit::Width;
                        let fit_icon = if is_width { "maximize" } else { "maximize-2" };
                        let fit_tip = if is_width { tl!("Zoom to page level") } else { tl!("Fit to width") };
                        if icons::button(ui, fit_icon, 28.0, false, &fit_tip).clicked() {
                            view.fit = if is_width { Fit::Page } else { Fit::Width };
                            view.goto = Some((view.current, 0.0));
                        }
                    });
                });
        });
}

/// Render modal drawers (Menu, Tools, Inspector) with scrim backdrop.
fn show_modal_sheet(app: &mut PdfCraftApp, sheet: MobileSheet, ui: &mut egui::Ui, t: &Tokens) {
    let screen = ui.ctx().content_rect();

    // 1. Full-screen scrim backdrop: tap anywhere outside drawer to dismiss
    egui::Area::new(egui::Id::new("mobile_scrim"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.left_top())
        .show(ui.ctx(), |ui| {
            let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(150));
            if resp.clicked() {
                app.mobile_sheet = None;
            }
        });

    if app.mobile_sheet.is_none() {
        return;
    }

    // 2. Render Drawer Content
    match sheet {
        MobileSheet::Menu => show_menu_drawer(app, ui, t, screen),
        MobileSheet::Tools => show_tools_drawer(app, ui, t, screen),
        MobileSheet::Inspector => show_inspector_drawer(app, ui, t, screen),
    }
}

/// Slide-in hamburger menu from the left edge.
fn show_menu_drawer(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens, screen: Rect) {
    let drawer_width = (screen.width() * 0.85).min(320.0);
    let drawer_rect = Rect::from_min_size(screen.left_top(), vec2(drawer_width, screen.height()));

    egui::Area::new(egui::Id::new("mobile_menu_drawer"))
        .order(egui::Order::Foreground)
        .fixed_pos(drawer_rect.left_top())
        .show(ui.ctx(), |ui| {
            egui::Frame::NONE
                .fill(t.chrome)
                .stroke(Stroke::new(1.0, t.divider))
                .corner_radius(CornerRadius { nw: 0, sw: 0, ne: 12, se: 12 })
                .shadow(egui::Shadow {
                    offset: [6, 0],
                    blur: 24,
                    spread: 0,
                    color: Color32::from_black_alpha(if t.dark() { 140 } else { 45 }),
                })
                .inner_margin(egui::Margin::symmetric(14, 16))
                .show(ui, |ui| {
                    ui.set_width(drawer_width - 28.0);
                    ui.set_height(screen.height() - 32.0);

                    // Header row: Logo & Close button
                    ui.horizontal(|ui| {
                        widgets::artcraft_mark(ui, 24.0);
                        ui.label(egui::RichText::new("PdfCraft").font(theme::semibold(17.0)).color(t.text));

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if icons::button(ui, "x", 28.0, false, tl!("Close")).clicked() {
                                app.mobile_sheet = None;
                            }
                        });
                    });

                    ui.add_space(14.0);
                    ui.separator();
                    ui.add_space(8.0);

                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        // Section: Open Tabs / Documents
                        ui.label(egui::RichText::new(tl!("File")).font(theme::semibold(12.0)).color(t.text_muted));
                        ui.add_space(4.0);

                        let mut close_tab_idx = None;
                        let mut select_tab_idx = None;

                        // Home item
                        let is_home = app.active.is_none();
                        if ui.selectable_label(is_home, format!("🏠 {}", tl!("Home"))).clicked() {
                            app.active = None;
                            app.mobile_sheet = None;
                        }

                        // Open Document Tabs
                        for i in 0..app.views.len() {
                            let is_active = app.active == Some(i);
                            let name = crate::canvas::tab_label(app, i).unwrap_or_else(|| format!("Doc {}", i + 1));

                            ui.horizontal(|ui| {
                                if ui.selectable_label(is_active, &name).clicked() {
                                    select_tab_idx = Some(i);
                                }
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if icons::button(ui, "x", 22.0, false, tl!("Close")).clicked() {
                                        close_tab_idx = Some(i);
                                    }
                                });
                            });
                        }

                        if let Some(i) = select_tab_idx {
                            app.active = Some(i);
                            app.mobile_sheet = None;
                        }
                        if let Some(i) = close_tab_idx {
                            app.close_tab(i);
                        }

                        ui.add_space(4.0);
                        if ui.button(format!("+ {}", tl!("Open…"))).clicked() {
                            app.open_dialog();
                            app.mobile_sheet = None;
                        }

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // Section: Modes
                        ui.label(egui::RichText::new(tl!("View")).font(theme::semibold(12.0)).color(t.text_muted));
                        ui.add_space(4.0);

                        for (mode, label) in [
                            (Mode::Read, "Read"),
                            (Mode::Edit, "Edit"),
                            (Mode::AllTools, "All tools"),
                            (Mode::Sign, "E-Sign"),
                        ] {
                            let active = app.mode == mode;
                            if ui.selectable_label(active, tl!(label)).clicked() {
                                app.switch_mode(mode, true);
                                app.mobile_sheet = None;
                            }
                        }

                        // Section: File Commands (if active doc)
                        if app.active.is_some() {
                            ui.add_space(12.0);
                            ui.separator();
                            ui.add_space(8.0);

                            ui.label(egui::RichText::new(tl!("File")).font(theme::semibold(12.0)).color(t.text_muted));
                            ui.add_space(4.0);

                            if ui.button(tl!("Save")).clicked() {
                                app.run_command("file.save");
                                app.mobile_sheet = None;
                            }
                            if ui.button(tl!("Print")).clicked() {
                                app.run_command("print.dialog");
                                app.mobile_sheet = None;
                            }
                            if ui.button(tl!("Document properties…")).clicked() {
                                app.dialog = Some(Dialog::Properties(PropsTab::Description));
                                app.mobile_sheet = None;
                            }
                        }

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // Section: Theme & Preferences
                        ui.label(egui::RichText::new(tl!("Appearance")).font(theme::semibold(12.0)).color(t.text_muted));
                        ui.add_space(4.0);

                        let dark = t.dark();
                        let toggle_label = if dark { "☀️ Light" } else { "🌙 Dark" };
                        if ui.button(toggle_label).clicked() {
                            let new_pref = if dark { ThemePreference::Light } else { ThemePreference::Dark };
                            app.set_theme_preference(new_pref);
                        }
                    });
                });
        });
}

/// Drawer / Sheet for mobile editing and annotation tools.
fn show_tools_drawer(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens, screen: Rect) {
    let drawer_width = (screen.width() * 0.88).min(330.0);
    let drawer_rect = Rect::from_min_size(screen.left_top(), vec2(drawer_width, screen.height()));

    egui::Area::new(egui::Id::new("mobile_tools_drawer"))
        .order(egui::Order::Foreground)
        .fixed_pos(drawer_rect.left_top())
        .show(ui.ctx(), |ui| {
            egui::Frame::NONE
                .fill(t.chrome)
                .stroke(Stroke::new(1.0, t.divider))
                .corner_radius(CornerRadius { nw: 0, sw: 0, ne: 12, se: 12 })
                .shadow(egui::Shadow {
                    offset: [6, 0],
                    blur: 24,
                    spread: 0,
                    color: Color32::from_black_alpha(if t.dark() { 140 } else { 45 }),
                })
                .inner_margin(egui::Margin::symmetric(14, 16))
                .show(ui, |ui| {
                    ui.set_width(drawer_width - 28.0);
                    ui.set_height(screen.height() - 32.0);

                    // Header row
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(tl!("All tools")).font(theme::semibold(17.0)).color(t.text));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if icons::button(ui, "x", 28.0, false, tl!("Close")).clicked() {
                                app.mobile_sheet = None;
                            }
                        });
                    });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(8.0);

                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        // Quick tool buttons
                        ui.label(egui::RichText::new(tl!("All tools")).font(theme::semibold(12.0)).color(t.text_muted));
                        ui.add_space(6.0);

                        let tools: &[(QuickTool, &str, &str)] = &[
                            (QuickTool::Select, "mouse-pointer-2", tl!("Select (V)")),
                            (QuickTool::Hand, "hand", tl!("Hand (H)")),
                            (QuickTool::Comment(CommentTool::Highlight), "highlighter", tl!(CommentTool::Highlight.label())),
                            (QuickTool::Comment(CommentTool::Note), "message-square", tl!(CommentTool::Note.label())),
                            (QuickTool::Comment(CommentTool::Ink), "brush", tl!(CommentTool::Ink.label())),
                            (QuickTool::Comment(CommentTool::Underline), "underline", tl!(CommentTool::Underline.label())),
                            (QuickTool::AddText, "type", tl!("Add text")),
                            (QuickTool::EditText, "pencil", tl!("Edit text")),
                        ];

                        for (tool, icon, label) in tools {
                            let is_active = app.quick_tool == *tool;
                            ui.horizontal(|ui| {
                                icons::paint(ui, Rect::from_min_size(ui.cursor().min, vec2(20.0, 20.0)), icon, 18.0, if is_active { t.accent } else { t.text });
                                ui.add_space(24.0);
                                if ui.selectable_label(is_active, *label).clicked() {
                                    app.quick_tool = *tool;
                                    app.mobile_sheet = None;
                                }
                            });
                            ui.add_space(4.0);
                        }

                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // Mode catalog
                        panels::left_panel(app, ui);
                    });
                });
        });
}

/// Slide-in Inspector drawer (Thumbnails, Bookmarks, Comments) from the right edge.
fn show_inspector_drawer(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens, screen: Rect) {
    let drawer_width = (screen.width() * 0.88).min(330.0);
    let drawer_x = screen.right() - drawer_width;
    let drawer_rect = Rect::from_min_size(egui::pos2(drawer_x, screen.top()), vec2(drawer_width, screen.height()));

    egui::Area::new(egui::Id::new("mobile_inspector_drawer"))
        .order(egui::Order::Foreground)
        .fixed_pos(drawer_rect.left_top())
        .show(ui.ctx(), |ui| {
            egui::Frame::NONE
                .fill(t.chrome)
                .stroke(Stroke::new(1.0, t.divider))
                .corner_radius(CornerRadius { nw: 12, sw: 12, ne: 0, se: 0 })
                .shadow(egui::Shadow {
                    offset: [-6, 0],
                    blur: 24,
                    spread: 0,
                    color: Color32::from_black_alpha(if t.dark() { 140 } else { 45 }),
                })
                .inner_margin(egui::Margin::symmetric(14, 16))
                .show(ui, |ui| {
                    ui.set_width(drawer_width - 28.0);
                    ui.set_height(screen.height() - 32.0);

                    // Header row: Tabs & Close
                    ui.horizontal(|ui| {
                        let current_panel = app.right.unwrap_or(RightPanel::Pages);
                        if ui.selectable_label(current_panel == RightPanel::Pages, tl!("Pages")).clicked() {
                            app.right = Some(RightPanel::Pages);
                        }
                        if ui.selectable_label(current_panel == RightPanel::Bookmarks, tl!("Bookmarks")).clicked() {
                            app.right = Some(RightPanel::Bookmarks);
                        }
                        if ui.selectable_label(current_panel == RightPanel::Comments, tl!("Comments")).clicked() {
                            app.right = Some(RightPanel::Comments);
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if icons::button(ui, "x", 28.0, false, tl!("Close")).clicked() {
                                app.mobile_sheet = None;
                            }
                        });
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // Panel Content
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        panels::right_panel(app, ui);
                    });
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_sheet_transitions() {
        let mut app = PdfCraftApp::new();
        assert_eq!(app.mobile_sheet, None);
        app.mobile_sheet = Some(MobileSheet::Menu);
        assert_eq!(app.mobile_sheet, Some(MobileSheet::Menu));
        app.mobile_sheet = Some(MobileSheet::Tools);
        assert_eq!(app.mobile_sheet, Some(MobileSheet::Tools));
        app.mobile_sheet = Some(MobileSheet::Inspector);
        assert_eq!(app.mobile_sheet, Some(MobileSheet::Inspector));
        app.mobile_sheet = None;
        assert_eq!(app.mobile_sheet, None);
    }
}

