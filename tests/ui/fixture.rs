//! Test fixture: a `TableView` + `VariantTable` driven headlessly by egui_kittest.
//!
//! Tests should go through these helpers instead of reaching into the view, so that the
//! DESIGN-8 rework only has to port this file. See AGENTS.md ("UI tests") for conventions.

use egui::{Event, Key, Modifiers, OutputCommand, Pos2, Vec2, accesskit::Role};
use egui_kittest::kittest::Queryable;
use egui_kittest::{Harness, HarnessBuilder, Node};
use egui_tabular::rvariant::{Variant, VariantTy};
use egui_tabular::{
    CellCoord, CellUi, ColumnDef, ColumnUid, RowUid, TableEvent, TableModel, TableView,
    TableViewConfig, TableViewOptions, VariantCellUi, VariantTable,
};

/// Horizontal cell padding: half of egui's default `item_spacing.x`.
const PAD_X: f32 = 4.0;

pub struct App<M = VariantTable, C = VariantCellUi> {
    pub table: M,
    pub cell_ui: C,
    pub view: TableView,
    pub config: TableViewConfig,
    /// Text the table put on the clipboard during the last frame that copied anything.
    pub copied: Option<String>,
    /// Every event the view reported, oldest first.
    pub events: Vec<TableEvent>,
}

/// A table over a model `M` shown with `C`. Defaults to [`Table::grid`]'s `VariantTable`.
pub struct Table<M: 'static = VariantTable, C: 'static = VariantCellUi> {
    pub h: Harness<'static, App<M, C>>,
    pub rows: Vec<RowUid>,
    pub cols: Vec<ColumnUid>,
}

/// Spreadsheet-style name of a column: 0 → "A", 1 → "B", ...
pub fn col_name(col: usize) -> String {
    char::from(b'A' + col as u8).to_string()
}

/// Text of a cell in a [`Table::grid`]: column letter and 1-based row number, e.g. "B2".
pub fn cell_text(col: usize, row: usize) -> String {
    format!("{}{}", col_name(col), row + 1)
}

/// Modifiers for Ctrl+key on Linux/Windows (`command` mirrors `ctrl` there).
pub const CTRL: Modifiers = Modifiers {
    ctrl: true,
    command: true,
    ..Modifiers::NONE
};

impl Table {
    /// `cols` string columns named "A", "B", ... and `rows` rows. Every cell holds its
    /// spreadsheet name ("A1", "B1", ...), so it can be found by its text.
    pub fn grid(cols: usize, rows: usize) -> Self {
        Self::grid_with(
            Harness::builder().with_size(Vec2::new(600.0, 400.0)),
            cols,
            rows,
        )
    }

    /// [`Table::grid`] with a customized harness, e.g. `.with_os(..)` or `.with_size(..)`.
    pub fn grid_with(builder: HarnessBuilder<App>, cols: usize, rows: usize) -> Self {
        let mut table =
            VariantTable::new((0..cols).map(|c| ColumnDef::new(col_name(c), VariantTy::Str)));
        for r in 0..rows {
            table.insert_row(
                (0..cols).map(|c| (ColumnUid(c as u32), Variant::Str(cell_text(c, r)))),
            );
        }
        Self::build(builder, table, VariantCellUi)
    }
}

impl<M: TableModel + 'static, C: CellUi<M> + 'static> Table<M, C> {
    /// Any model, shown with `cell_ui` in a 600×400 harness.
    pub fn custom(table: M, cell_ui: C) -> Self {
        Self::build(
            Harness::builder().with_size(Vec2::new(600.0, 400.0)),
            table,
            cell_ui,
        )
    }

    pub fn build(builder: HarnessBuilder<App<M, C>>, table: M, cell_ui: C) -> Self {
        let rows = table.rows().collect();
        let cols = table.columns().collect();
        let app = App {
            table,
            cell_ui,
            view: TableView::new(TableViewOptions::default()),
            config: TableViewConfig::default(),
            copied: None,
            events: vec![],
        };
        let h = builder.build_ui_state(
            |ui, app: &mut App<M, C>| {
                let output = app
                    .view
                    .show(ui, &mut app.table, &mut app.cell_ui, &mut app.config);
                app.events.extend(output.events);
                let copied = ui.ctx().output(|o| {
                    o.commands.iter().rev().find_map(|c| match c {
                        OutputCommand::CopyText(text) => Some(text.clone()),
                        _ => None,
                    })
                });
                if copied.is_some() {
                    app.copied = copied;
                }
            },
            app,
        );
        Self { h, rows, cols }
    }

    // ---- Queries ----

    /// The node of a label with exactly this text (a cell value, header name or row number).
    /// Panics if there is none or more than one.
    pub fn node<'a>(&'a self, text: &'a str) -> Node<'a> {
        self.h.get_by_label(text)
    }

    /// Whether exactly one label with this text is shown.
    pub fn has(&self, text: &str) -> bool {
        self.h.query_by_label(text).is_some()
    }

    pub fn center(&self, text: &str) -> Pos2 {
        self.node(text).rect().center()
    }

    /// The open cell editor, if any.
    pub fn editor(&self) -> Option<Node<'_>> {
        self.h.query_by_role(Role::TextInput)
    }

    /// The editor's current text. Panics if no editor is open.
    pub fn editor_text(&self) -> String {
        self.editor()
            .expect("no cell editor is open")
            .value()
            .unwrap_or_default()
    }

    /// Model value of a cell as a string.
    pub fn value(&self, row: usize, col: usize) -> Option<String> {
        let coord = CellCoord {
            row_uid: self.rows[row],
            col_uid: self.cols[col],
        };
        self.h.state().table.get(coord).map(|v| v.to_string())
    }

    /// Data column names in visual order, read from header label positions.
    pub fn header_order(&self) -> Vec<String> {
        // Hidden columns have no header; a menu's checkbox of the same name is not a Label.
        let mut headers: Vec<_> = (0..self.cols.len())
            .filter_map(|c| {
                let name = col_name(c);
                let node = self
                    .h
                    .query_all_by_role_and_label(Role::Label, &name)
                    .next()?;
                Some((node.rect().min.x, name))
            })
            .collect();
        headers.sort_by(|a, b| a.0.total_cmp(&b.0));
        headers.into_iter().map(|(_, name)| name).collect()
    }

    /// Labels from `texts` that are shown, top to bottom: the visible row order of those cells.
    pub fn top_down(&self, texts: &[&str]) -> Vec<String> {
        let mut shown: Vec<_> = texts
            .iter()
            .filter_map(|t| {
                self.h
                    .query_by_label(t)
                    .map(|n| (n.rect().min.y, t.to_string()))
            })
            .collect();
        shown.sort_by(|a, b| a.0.total_cmp(&b.0));
        shown.into_iter().map(|(_, t)| t).collect()
    }

    /// Change the view config the way an app does between frames, and run.
    pub fn config(&mut self, change: impl FnOnce(&mut TableViewConfig)) {
        change(&mut self.h.state_mut().config);
        self.h.run();
    }

    /// Change the view's options (`TableViewOptions`) and redraw.
    pub fn options(&mut self, change: impl FnOnce(&mut TableViewOptions)) {
        change(self.h.state_mut().view.options_mut());
        self.h.run();
    }

    /// Whether the view reported an event matching `f`.

    pub fn saw(&self, f: impl Fn(&TableEvent) -> bool) -> bool {
        self.h.state().events.iter().any(f)
    }

    /// x of the right edge (resize handle) of data column `col`, from the next header's position.
    pub fn column_right_edge(&self, col: usize) -> f32 {
        self.node(&col_name(col + 1)).rect().min.x - PAD_X
    }

    /// Ctrl+C, returning what the table copied (`None` if nothing). This is the black-box way
    /// to check the current selection.
    pub fn copy(&mut self) -> Option<String> {
        self.h.state_mut().copied = None;
        self.press_copy();
        self.h.state().copied.clone()
    }

    // ---- Actions ----
    // Every action runs the harness until the UI settles.

    /// Click the label with this text (a cell value, header or row number).
    pub fn click(&mut self, text: &str) {
        self.node(text).click();
        self.h.run();
    }

    /// Right-click the label with this text, e.g. to open a context menu.
    pub fn right_click(&mut self, text: &str) {
        self.node(text).click_secondary();
        self.h.run();
    }

    pub fn shift_click(&mut self, text: &str) {
        self.node(text).click_modifiers(Modifiers::SHIFT);
        self.h.run();
    }

    /// Click a cell until it is in edit mode: once if it is already selected, otherwise twice.
    pub fn start_edit(&mut self, text: &str) {
        self.click(text);
        if self.editor().is_none() {
            self.click(text);
        }
        assert!(
            self.editor().is_some(),
            "clicking {text} twice didn't open an editor"
        );
    }

    /// Replace the open editor's text.
    pub fn type_replace(&mut self, text: &str) {
        self.ctrl(Key::A);
        self.editor()
            .expect("no cell editor is open")
            .type_text(text);
        self.h.run();
    }

    pub fn press(&mut self, key: Key) {
        self.h.key_press(key);
        self.h.run();
    }

    pub fn press_modifiers(&mut self, modifiers: Modifiers, key: Key) {
        self.h.key_press_modifiers(modifiers, key);
        self.h.run();
    }

    /// Ctrl+C (Cmd+C on macOS) as egui-winit and eframe's web runner deliver it: `Event::Copy`,
    /// no key press.
    /// The copied text is read from the frame's whole output: egui copies selected label text at
    /// the end of the pass, after the app's UI ran.
    pub fn press_copy(&mut self) {
        self.h.event(Event::Copy);
        self.h.step();
        let copied = self
            .h
            .output()
            .platform_output
            .commands
            .iter()
            .rev()
            .find_map(|c| match c {
                OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            });
        if copied.is_some() {
            self.h.state_mut().copied = copied;
        }
        self.h.run();
    }

    pub fn ctrl(&mut self, key: Key) {
        self.press_modifiers(CTRL, key);
    }

    /// Move the pointer over a label. Keyboard input only reaches the table while the pointer
    /// is over it (SEL-4); clicks leave the pointer in place.
    pub fn hover(&mut self, text: &str) {
        self.node(text).hover();
        self.h.run();
    }

    /// Scroll just enough to bring the row at natural index `row` into view
    /// (`TableView::scroll_to_row`).
    pub fn scroll_to_row(&mut self, row: usize) {
        let uid = self.rows[row];
        self.h.state_mut().view.scroll_to_row(uid);
        self.h.run();
    }

    /// Move the pointer off the table.
    pub fn pointer_away(&mut self) {
        self.h.remove_cursor();
        self.h.run();
    }

    pub fn paste(&mut self, text: &str) {
        self.h.event(Event::Paste(text.to_owned()));
        self.h.run();
    }

    /// Click at a position. Unlike `Node::click`, works on things without an AccessKit node.
    pub fn click_at(&mut self, pos: Pos2) {
        self.press_release_at(pos);
        self.h.run();
    }

    /// Each queued event gets its own 0.25 s frame, which is longer than egui's double-click
    /// window, so both clicks go into one frame's raw input instead.
    pub fn double_click_at(&mut self, pos: Pos2) {
        self.h.hover_at(pos);
        self.h.step();
        for _ in 0..2 {
            for pressed in [true, false] {
                self.h.input_mut().events.push(Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                });
            }
        }
        self.h.run();
    }

    /// Right-click at a position (opens a context menu).
    pub fn right_click_at(&mut self, pos: Pos2) {
        self.h.hover_at(pos);
        self.h.step();
        for pressed in [true, false] {
            self.h.event(Event::PointerButton {
                pos,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        self.h.run();
    }

    /// Right-click the tool column header (the table icon, above the row numbers).
    pub fn right_click_tool_header(&mut self) {
        let y = self.node("A").rect().center().y;
        let x = self.node("0").rect().center().x;
        self.right_click_at(egui::pos2(x, y));
    }

    /// Click the checkbox with this label (e.g. in the column chooser).
    pub fn click_checkbox(&mut self, text: &str) {
        self.h.get_by_role_and_label(Role::CheckBox, text).click();
        self.h.run();
    }

    fn press_release_at(&mut self, pos: Pos2) {
        self.h.hover_at(pos);
        for pressed in [true, false] {
            self.h.event(Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
    }

    /// Press at `from`, move to `to` in a few frames (past egui's drag threshold), release.
    pub fn drag(&mut self, from: Pos2, to: Pos2) {
        self.h.hover_at(from);
        self.h.drag_at(from);
        for t in [0.1, 0.5, 1.0] {
            self.h.hover_at(from.lerp(to, t));
        }
        self.h.drop_at(to);
        self.h.run();
    }

    /// Drag the header of column `from` and drop it on the header of column `to`.
    pub fn drag_column(&mut self, from: usize, to: usize) {
        let (from, to) = (self.center(&col_name(from)), self.center(&col_name(to)));
        self.drag(from, to);
    }
}
