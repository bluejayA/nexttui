use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

const MAX_INPUT_LEN: usize = 256;
const POPUP_VISIBLE_ITEMS: usize = 10;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Display option for Dropdown / MultiSelect fields.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectOption {
    pub value: String,
    pub display: String,
}

impl SelectOption {
    pub fn new(value: impl Into<String>, display: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            display: display.into(),
        }
    }

    /// Convenience: same string for value and display.
    pub fn simple(s: impl Into<String>) -> Self {
        let s = s.into();
        Self {
            value: s.clone(),
            display: s,
        }
    }
}

/// Validation rules attachable to a field.
#[derive(Debug, Clone, PartialEq)]
pub enum Validation {
    Required,
    MinLength(usize),
    MaxLength(usize),
    Numeric,
    Cidr,
}

/// Validation error for a single field.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldError {
    pub field_name: String,
    pub message: String,
}

/// Result value for a single field on submit.
#[derive(Debug, Clone, PartialEq)]
pub enum FormValue {
    Text(String),
    Selected(String),
    MultiSelected(Vec<String>),
    Bool(bool),
}

/// All field values keyed by field name.
pub type FormValues = HashMap<String, FormValue>;

// ---------------------------------------------------------------------------
// FieldDef — immutable field definition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum FieldDef {
    Text {
        name: String,
        label: String,
        placeholder: String,
        validations: Vec<Validation>,
        password: bool,
    },
    Dropdown {
        name: String,
        label: String,
        validations: Vec<Validation>,
        options: Vec<SelectOption>,
    },
    MultiSelect {
        name: String,
        label: String,
        validations: Vec<Validation>,
        options: Vec<SelectOption>,
    },
    Checkbox {
        name: String,
        label: String,
    },
}

impl FieldDef {
    pub fn name(&self) -> &str {
        match self {
            FieldDef::Text { name, .. }
            | FieldDef::Dropdown { name, .. }
            | FieldDef::MultiSelect { name, .. }
            | FieldDef::Checkbox { name, .. } => name,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            FieldDef::Text { label, .. }
            | FieldDef::Dropdown { label, .. }
            | FieldDef::MultiSelect { label, .. }
            | FieldDef::Checkbox { label, .. } => label,
        }
    }

    pub fn validations(&self) -> &[Validation] {
        match self {
            FieldDef::Text { validations, .. }
            | FieldDef::Dropdown { validations, .. }
            | FieldDef::MultiSelect { validations, .. } => validations,
            FieldDef::Checkbox { .. } => &[],
        }
    }

    // -- Builder helpers (compatible signature for easy migration) -----------

    pub fn text(label: impl Into<String>, required: bool) -> Self {
        let label = label.into();
        let name = label.clone();
        let mut validations = Vec::new();
        if required {
            validations.push(Validation::Required);
        }
        FieldDef::Text {
            name,
            label,
            placeholder: String::new(),
            validations,
            password: false,
        }
    }

    pub fn text_with_name(
        name: impl Into<String>,
        label: impl Into<String>,
        required: bool,
    ) -> Self {
        let mut validations = Vec::new();
        if required {
            validations.push(Validation::Required);
        }
        FieldDef::Text {
            name: name.into(),
            label: label.into(),
            placeholder: String::new(),
            validations,
            password: false,
        }
    }

    pub fn password(label: impl Into<String>, required: bool) -> Self {
        let label = label.into();
        let name = label.clone();
        let mut validations = Vec::new();
        if required {
            validations.push(Validation::Required);
        }
        FieldDef::Text {
            name,
            label,
            placeholder: String::new(),
            validations,
            password: true,
        }
    }

    pub fn dropdown(
        label: impl Into<String>,
        options: Vec<String>,
        required: bool,
    ) -> Self {
        let label = label.into();
        let name = label.clone();
        let mut validations = Vec::new();
        if required {
            validations.push(Validation::Required);
        }
        FieldDef::Dropdown {
            name,
            label,
            validations,
            options: options.into_iter().map(SelectOption::simple).collect(),
        }
    }

    pub fn dropdown_with_options(
        name: impl Into<String>,
        label: impl Into<String>,
        options: Vec<SelectOption>,
        required: bool,
    ) -> Self {
        let mut validations = Vec::new();
        if required {
            validations.push(Validation::Required);
        }
        FieldDef::Dropdown {
            name: name.into(),
            label: label.into(),
            validations,
            options,
        }
    }

    pub fn multiselect(
        label: impl Into<String>,
        options: Vec<SelectOption>,
    ) -> Self {
        let label = label.into();
        let name = label.clone();
        FieldDef::MultiSelect {
            name,
            label,
            validations: Vec::new(),
            options,
        }
    }

    pub fn checkbox(label: impl Into<String>) -> Self {
        let label = label.into();
        let name = label.clone();
        FieldDef::Checkbox { name, label }
    }
}

// ---------------------------------------------------------------------------
// FieldState — mutable runtime state per field
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum FieldState {
    Text {
        value: String,
        cursor: usize,
    },
    Dropdown {
        selected: Option<usize>,
        open: bool,
        scroll: usize,
    },
    MultiSelect {
        selected: Vec<bool>,
        open: bool,
        scroll: usize,
    },
    Checkbox {
        checked: bool,
    },
}

impl FieldState {
    /// Create default state matching a FieldDef.
    pub fn default_for(def: &FieldDef) -> Self {
        match def {
            FieldDef::Text { .. } => FieldState::Text {
                value: String::new(),
                cursor: 0,
            },
            FieldDef::Dropdown { .. } => FieldState::Dropdown {
                selected: None,
                open: false,
                scroll: 0,
            },
            FieldDef::MultiSelect { options, .. } => FieldState::MultiSelect {
                selected: vec![false; options.len()],
                open: false,
                scroll: 0,
            },
            FieldDef::Checkbox { .. } => FieldState::Checkbox { checked: false },
        }
    }
}

// ---------------------------------------------------------------------------
// FormAction
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum FormAction {
    Submit(FormValues),
    Cancel,
    None,
}

// ---------------------------------------------------------------------------
// FormPhase
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormPhase {
    Editing,
    Confirming,
}

// ---------------------------------------------------------------------------
// FormWidget
// ---------------------------------------------------------------------------

pub struct FormWidget {
    title: String,
    fields: Vec<(FieldDef, FieldState)>,
    focused: usize,
    errors: Vec<FieldError>,
    phase: FormPhase,
}

impl FormWidget {
    pub fn new(title: impl Into<String>, defs: Vec<FieldDef>) -> Self {
        debug_assert!(
            {
                let mut names: Vec<&str> = defs.iter().map(|d| d.name()).collect();
                let total = names.len();
                names.sort();
                names.dedup();
                names.len() == total
            },
            "FormWidget field names must be unique"
        );
        let fields: Vec<(FieldDef, FieldState)> = defs
            .into_iter()
            .map(|d| {
                let s = FieldState::default_for(&d);
                (d, s)
            })
            .collect();
        Self {
            title: title.into(),
            fields,
            focused: 0,
            errors: Vec::new(),
            phase: FormPhase::Editing,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn phase(&self) -> FormPhase {
        self.phase
    }

    fn label_width(&self, max_width: u16) -> usize {
        self.fields
            .iter()
            .map(|(d, _)| d.label().chars().count())
            .max()
            .unwrap_or(0)
            .min(max_width as usize / 3)
    }

    pub fn fields(&self) -> &[(FieldDef, FieldState)] {
        &self.fields
    }

    pub fn focused_index(&self) -> usize {
        self.focused
    }

    pub fn focused_field_name(&self) -> &str {
        self.fields
            .get(self.focused)
            .map(|(d, _)| d.name())
            .unwrap_or("")
    }

    pub fn errors(&self) -> &[FieldError] {
        &self.errors
    }

    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    // -- Dynamic setters ----------------------------------------------------

    pub fn set_field_options(&mut self, name: &str, options: Vec<SelectOption>) {
        for (def, state) in &mut self.fields {
            if def.name() == name {
                match def {
                    FieldDef::Dropdown {
                        options: opts,
                        ..
                    } => {
                        *opts = options;
                        *state = FieldState::Dropdown {
                            selected: None,
                            open: false,
                            scroll: 0,
                        };
                    }
                    FieldDef::MultiSelect {
                        options: opts,
                        ..
                    } => {
                        let len = options.len();
                        *opts = options;
                        *state = FieldState::MultiSelect {
                            selected: vec![false; len],
                            open: false,
                            scroll: 0,
                        };
                    }
                    _ => {}
                }
                return;
            }
        }
    }

    pub fn set_field_value(&mut self, name: &str, value: FormValue) {
        for (def, state) in &mut self.fields {
            if def.name() != name {
                continue;
            }
            match (def, state, &value) {
                (FieldDef::Text { .. }, FieldState::Text { value: v, cursor }, FormValue::Text(t)) => {
                    let truncated: String = t.chars().take(MAX_INPUT_LEN).collect();
                    *cursor = truncated.chars().count();
                    *v = truncated;
                }
                (
                    FieldDef::Dropdown { options, .. },
                    FieldState::Dropdown { selected, .. },
                    FormValue::Selected(val),
                ) => {
                    *selected = options.iter().position(|o| o.value == *val);
                }
                (
                    FieldDef::MultiSelect { options, .. },
                    FieldState::MultiSelect { selected, .. },
                    FormValue::MultiSelected(vals),
                ) => {
                    for (i, opt) in options.iter().enumerate() {
                        if i < selected.len() {
                            selected[i] = vals.contains(&opt.value);
                        }
                    }
                }
                (FieldDef::Checkbox { .. }, FieldState::Checkbox { checked }, FormValue::Bool(b)) => {
                    *checked = *b;
                }
                _ => {}
            }
            return;
        }
    }

    // -- Validation ---------------------------------------------------------

    fn validate_field(def: &FieldDef, state: &FieldState) -> Option<FieldError> {
        let name = def.name().to_string();
        for rule in def.validations() {
            let err = match (rule, def, state) {
                (Validation::Required, FieldDef::Text { .. }, FieldState::Text { value, .. }) => {
                    if value.trim().is_empty() {
                        Some(format!("{} is required", def.label()))
                    } else {
                        None
                    }
                }
                (
                    Validation::Required,
                    FieldDef::Dropdown { .. },
                    FieldState::Dropdown { selected, .. },
                ) => {
                    if selected.is_none() {
                        Some(format!("{} is required", def.label()))
                    } else {
                        None
                    }
                }
                (
                    Validation::Required,
                    FieldDef::MultiSelect { .. },
                    FieldState::MultiSelect { selected, .. },
                ) => {
                    if !selected.iter().any(|&s| s) {
                        Some(format!("{} is required", def.label()))
                    } else {
                        None
                    }
                }
                (Validation::MinLength(min), FieldDef::Text { .. }, FieldState::Text { value, .. }) => {
                    let char_count = value.chars().count();
                    if !value.is_empty() && char_count < *min {
                        Some(format!("{} must be at least {} characters", def.label(), min))
                    } else {
                        None
                    }
                }
                (Validation::MaxLength(max), FieldDef::Text { .. }, FieldState::Text { value, .. }) => {
                    if value.chars().count() > *max {
                        Some(format!("{} must be at most {} characters", def.label(), max))
                    } else {
                        None
                    }
                }
                (Validation::Numeric, FieldDef::Text { .. }, FieldState::Text { value, .. }) => {
                    if !value.is_empty() && value.parse::<f64>().is_err() {
                        Some(format!("{} must be numeric", def.label()))
                    } else {
                        None
                    }
                }
                (Validation::Cidr, FieldDef::Text { .. }, FieldState::Text { value, .. }) => {
                    if !value.is_empty() && !is_valid_ipv4_cidr(value) {
                        Some(format!("{} must be a valid CIDR (e.g. 10.0.0.0/24)", def.label()))
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(message) = err {
                return Some(FieldError {
                    field_name: name,
                    message,
                });
            }
        }
        None
    }

    pub fn validate_and_submit(&mut self) -> FormAction {
        self.errors.clear();
        for (def, state) in &self.fields {
            if let Some(err) = Self::validate_field(def, state) {
                self.errors.push(err);
            }
        }

        if !self.errors.is_empty() {
            // Focus first error field
            let first_err_name = &self.errors[0].field_name;
            if let Some(idx) = self
                .fields
                .iter()
                .position(|(d, _)| d.name() == first_err_name)
            {
                self.focused = idx;
            }
            return FormAction::None;
        }

        // Transition to Confirming phase instead of immediate submit
        self.phase = FormPhase::Confirming;
        FormAction::None
    }

    /// Confirm and submit from Confirming phase. Returns Submit with built values.
    pub fn confirm_submit(&mut self) -> FormAction {
        if self.phase != FormPhase::Confirming {
            return FormAction::None;
        }
        let values = self.build_values();
        self.phase = FormPhase::Editing;
        FormAction::Submit(values)
    }

    fn build_values(&self) -> FormValues {
        let mut values = FormValues::new();
        for (def, state) in &self.fields {
            let val = match (def, state) {
                (FieldDef::Text { .. }, FieldState::Text { value, .. }) => {
                    FormValue::Text(value.clone())
                }
                (
                    FieldDef::Dropdown { options, .. },
                    FieldState::Dropdown { selected, .. },
                ) => {
                    let v = selected
                        .and_then(|i| options.get(i))
                        .map(|o| o.value.clone())
                        .unwrap_or_default();
                    FormValue::Selected(v)
                }
                (
                    FieldDef::MultiSelect { options, .. },
                    FieldState::MultiSelect { selected, .. },
                ) => {
                    let vals: Vec<String> = options
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| selected.get(*i).copied().unwrap_or(false))
                        .map(|(_, o)| o.value.clone())
                        .collect();
                    FormValue::MultiSelected(vals)
                }
                (FieldDef::Checkbox { .. }, FieldState::Checkbox { checked }) => {
                    FormValue::Bool(*checked)
                }
                _ => continue,
            };
            values.insert(def.name().to_string(), val);
        }
        values
    }

    // -- Key handling -------------------------------------------------------

    fn is_any_popup_open(&self) -> bool {
        if let Some((_, state)) = self.fields.get(self.focused) {
            matches!(
                state,
                FieldState::Dropdown { open: true, .. }
                    | FieldState::MultiSelect { open: true, .. }
            )
        } else {
            false
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> FormAction {
        if self.phase == FormPhase::Confirming {
            return self.handle_confirming_key(key);
        }
        if self.is_any_popup_open() {
            self.handle_popup_key(key)
        } else {
            self.handle_form_key(key)
        }
    }

    fn handle_confirming_key(&mut self, key: KeyEvent) -> FormAction {
        match key.code {
            KeyCode::Enter => self.confirm_submit(),
            KeyCode::Esc | KeyCode::Left => {
                self.phase = FormPhase::Editing;
                FormAction::None
            }
            _ => FormAction::None,
        }
    }

    fn handle_form_key(&mut self, key: KeyEvent) -> FormAction {
        match key.code {
            KeyCode::Up | KeyCode::BackTab => {
                self.focused = self.focused.saturating_sub(1);
                FormAction::None
            }
            KeyCode::Down | KeyCode::Tab => {
                if self.focused < self.fields.len().saturating_sub(1) {
                    self.focused += 1;
                }
                FormAction::None
            }
            KeyCode::Left => FormAction::Cancel,
            KeyCode::Esc => FormAction::Cancel,
            KeyCode::Right | KeyCode::Enter => {
                self.handle_activate(key.code)
            }
            KeyCode::Char(' ') => {
                if let Some((def, state)) = self.fields.get_mut(self.focused) {
                    match (def, state) {
                        (FieldDef::Checkbox { .. }, FieldState::Checkbox { checked }) => {
                            *checked = !*checked;
                        }
                        (FieldDef::Text { .. }, FieldState::Text { value, cursor }) => {
                            if value.chars().count() < MAX_INPUT_LEN {
                                let byte_pos = char_to_byte_pos(value, *cursor);
                                value.insert(byte_pos, ' ');
                                *cursor += 1;
                            }
                        }
                        _ => {}
                    }
                }
                FormAction::None
            }
            KeyCode::Char(c) => {
                if let Some((FieldDef::Text { .. }, FieldState::Text { value, cursor })) =
                    self.fields.get_mut(self.focused)
                    && value.chars().count() < MAX_INPUT_LEN
                {
                    let byte_pos = char_to_byte_pos(value, *cursor);
                    value.insert(byte_pos, c);
                    *cursor += 1;
                }
                FormAction::None
            }
            KeyCode::Backspace => {
                if let Some((FieldDef::Text { .. }, FieldState::Text { value, cursor })) =
                    self.fields.get_mut(self.focused)
                    && *cursor > 0
                {
                    *cursor -= 1;
                    let byte_pos = char_to_byte_pos(value, *cursor);
                    let next_byte = char_to_byte_pos(value, *cursor + 1);
                    value.replace_range(byte_pos..next_byte, "");
                }
                FormAction::None
            }
            _ => FormAction::None,
        }
    }

    fn handle_activate(&mut self, code: KeyCode) -> FormAction {
        let is_last_field = self.focused == self.fields.len().saturating_sub(1);

        if let Some((def, state)) = self.fields.get_mut(self.focused) {
            match (def, state) {
                (
                    FieldDef::Dropdown { options, .. },
                    FieldState::Dropdown { open, .. },
                ) if !options.is_empty() => {
                    *open = true;
                    return FormAction::None;
                }
                (
                    FieldDef::MultiSelect { options, .. },
                    FieldState::MultiSelect { open, .. },
                ) if !options.is_empty() => {
                    *open = true;
                    return FormAction::None;
                }
                (FieldDef::Checkbox { .. }, FieldState::Checkbox { checked }) => {
                    *checked = !*checked;
                    // Checkbox toggle does not submit — use Tab to move then Enter on last
                    return FormAction::None;
                }
                _ => {}
            }
        }

        // Enter submits only on the last field (spec FR-11)
        if code == KeyCode::Enter && is_last_field {
            return self.validate_and_submit();
        }

        FormAction::None
    }

    fn handle_popup_key(&mut self, key: KeyEvent) -> FormAction {
        let is_last_field = self.focused == self.fields.len().saturating_sub(1);
        let mut confirm_close = false;

        let Some((def, state)) = self.fields.get_mut(self.focused) else {
            return FormAction::None;
        };

        match (def, state) {
            (
                FieldDef::Dropdown { options, .. },
                FieldState::Dropdown {
                    selected,
                    open,
                    scroll,
                },
            ) => {
                match key.code {
                    KeyCode::Up => {
                        let sel = selected.unwrap_or(0);
                        *selected = Some(sel.saturating_sub(1));
                        let s = selected.unwrap_or(0);
                        if s < *scroll {
                            *scroll = s;
                        }
                    }
                    KeyCode::Down => {
                        let max = options.len().saturating_sub(1);
                        let new_sel = match *selected {
                            None => 0,
                            Some(s) => s.saturating_add(1).min(max),
                        };
                        *selected = Some(new_sel);
                        if new_sel >= *scroll + POPUP_VISIBLE_ITEMS {
                            *scroll = new_sel.saturating_sub(POPUP_VISIBLE_ITEMS - 1);
                        }
                    }
                    KeyCode::Enter | KeyCode::Right => {
                        *open = false;
                        confirm_close = key.code == KeyCode::Enter;
                    }
                    KeyCode::Esc | KeyCode::Left => {
                        *open = false;
                    }
                    _ => {}
                }
            }
            (
                FieldDef::MultiSelect { options, .. },
                FieldState::MultiSelect {
                    selected,
                    open,
                    scroll,
                },
            ) => {
                match key.code {
                    KeyCode::Up => {
                        *scroll = scroll.saturating_sub(1);
                    }
                    KeyCode::Down => {
                        let max = options.len().saturating_sub(1);
                        *scroll = (*scroll).saturating_add(1).min(max);
                    }
                    KeyCode::Char(' ') => {
                        let idx = *scroll;
                        if idx < selected.len() {
                            selected[idx] = !selected[idx];
                        }
                    }
                    KeyCode::Enter | KeyCode::Right => {
                        *open = false;
                        confirm_close = key.code == KeyCode::Enter;
                    }
                    KeyCode::Esc | KeyCode::Left => {
                        *open = false;
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        // After closing popup with Enter on the last field, attempt submit
        if confirm_close && is_last_field {
            return self.validate_and_submit();
        }

        FormAction::None
    }

    // -- Rendering ----------------------------------------------------------

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if self.phase == FormPhase::Confirming {
            return self.render_confirm_view(frame, area);
        }
        let block = Block::default()
            .title(format!(" {} ", self.title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let label_width = self.label_width(inner.width);

        let mut y = inner.y;
        let error_map: HashMap<&str, &str> = self
            .errors
            .iter()
            .map(|e| (e.field_name.as_str(), e.message.as_str()))
            .collect();

        for (i, (def, state)) in self.fields.iter().enumerate() {
            if y >= inner.y + inner.height {
                break;
            }

            let is_focused = i == self.focused;
            let label_style = if is_focused {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            let is_required = def.validations().contains(&Validation::Required);
            let label_text = if is_required {
                format!("* {:>width$}: ", def.label(), width = label_width.saturating_sub(2))
            } else {
                format!("{:>width$}: ", def.label(), width = label_width)
            };
            let value_width = inner
                .width
                .saturating_sub(label_text.chars().count() as u16);

            // Render label + value on one line
            let value_spans = self.render_field_value(def, state, is_focused, value_width);
            let line = Line::from(
                std::iter::once(Span::styled(label_text, label_style))
                    .chain(value_spans)
                    .collect::<Vec<_>>(),
            );
            let line_area = Rect::new(inner.x, y, inner.width, 1);
            frame.render_widget(Paragraph::new(line), line_area);
            y += 1;

            // Show validation error below the field
            if let Some(err_msg) = error_map.get(def.name())
                && y < inner.y + inner.height
            {
                let padding = " ".repeat(label_width + 2);
                let err_line = Line::from(vec![
                    Span::raw(padding),
                    Span::styled(
                        format!("! {err_msg}"),
                        Style::default().fg(Color::Red),
                    ),
                ]);
                let err_area = Rect::new(inner.x, y, inner.width, 1);
                frame.render_widget(Paragraph::new(err_line), err_area);
                y += 1;
            }
        }

        // Render hint at bottom
        let hint_y = (inner.y + inner.height).saturating_sub(1);
        if hint_y > y {
            let has_required = self.fields.iter().any(|(d, _)| d.validations().contains(&Validation::Required));
            let hint_text = if has_required {
                " ↑↓ Navigate  ←/Esc Cancel  Enter Review  * = required"
            } else {
                " ↑↓ Navigate  ←/Esc Cancel  Enter Review"
            };
            let hint = Line::from(Span::styled(
                hint_text,
                Style::default().fg(Color::DarkGray),
            ));
            let hint_area = Rect::new(inner.x, hint_y, inner.width, 1);
            frame.render_widget(Paragraph::new(hint), hint_area);
        }

        // Render popup overlay (dropdown/multiselect) if open
        self.render_popup_overlay(frame, inner, label_width);
    }

    fn render_field_value<'a>(
        &self,
        def: &FieldDef,
        state: &FieldState,
        is_focused: bool,
        max_width: u16,
    ) -> Vec<Span<'a>> {
        match (def, state) {
            (FieldDef::Text { password, .. }, FieldState::Text { value, cursor }) => {
                let display: String = if *password {
                    "*".repeat(value.chars().count())
                } else {
                    value.clone()
                };
                let truncated: String = display.chars().take(max_width as usize).collect();

                if is_focused {
                    let cursor_pos = (*cursor).min(truncated.chars().count());
                    let before: String = truncated.chars().take(cursor_pos).collect();
                    let cursor_char: String = truncated
                        .chars()
                        .nth(cursor_pos)
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| " ".to_string());
                    let after: String = truncated.chars().skip(cursor_pos + 1).collect();

                    vec![
                        Span::styled(before, Style::default().fg(Color::White)),
                        Span::styled(
                            cursor_char,
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::White),
                        ),
                        Span::styled(after, Style::default().fg(Color::White)),
                    ]
                } else if truncated.is_empty() {
                    vec![Span::styled("(empty)", Style::default().fg(Color::DarkGray))]
                } else {
                    vec![Span::styled(truncated, Style::default().fg(Color::White))]
                }
            }
            (FieldDef::Dropdown { options, .. }, FieldState::Dropdown { selected, open, .. }) => {
                let display = selected
                    .and_then(|i| options.get(i))
                    .map(|o| o.display.clone())
                    .unwrap_or_else(|| "(select)".to_string());
                let arrow = if *open { " ▲" } else { " ▼" };
                let style = if is_focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };
                vec![
                    Span::styled(display, style),
                    Span::styled(arrow, Style::default().fg(Color::DarkGray)),
                ]
            }
            (
                FieldDef::MultiSelect { options, .. },
                FieldState::MultiSelect { selected, open, .. },
            ) => {
                let chosen: Vec<&str> = options
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| selected.get(*i).copied().unwrap_or(false))
                    .map(|(_, o)| o.display.as_str())
                    .collect();
                let display = if chosen.is_empty() {
                    "(select)".to_string()
                } else {
                    chosen.join(", ")
                };
                let arrow = if *open { " ▲" } else { " ▼" };
                let style = if is_focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };
                vec![
                    Span::styled(display, style),
                    Span::styled(arrow, Style::default().fg(Color::DarkGray)),
                ]
            }
            (FieldDef::Checkbox { .. }, FieldState::Checkbox { checked }) => {
                let icon = if *checked { "[x]" } else { "[ ]" };
                let style = if is_focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };
                vec![Span::styled(icon, style)]
            }
            _ => vec![],
        }
    }

    fn render_popup_overlay(&self, frame: &mut Frame, inner: Rect, label_width: usize) {
        let Some((def, state)) = self.fields.get(self.focused) else {
            return;
        };

        let label_cols = (label_width as u16).saturating_add(2);
        let popup_x = inner.x.saturating_add(label_cols);
        let available_width = inner.width.saturating_sub(label_cols);
        if available_width < 4 {
            return; // Not enough space for popup (need at least border + 2 chars)
        }

        // Calculate actual y position accounting for error lines above focused field
        let mut field_y = inner.y;
        let error_names: Vec<&str> = self.errors.iter().map(|e| e.field_name.as_str()).collect();
        for (i, (d, _)) in self.fields.iter().enumerate() {
            if i == self.focused {
                break;
            }
            field_y = field_y.saturating_add(1); // field line
            if error_names.contains(&d.name()) {
                field_y = field_y.saturating_add(1); // error line
            }
        }

        // Frame bounds for clamping
        let frame_w = frame.area().width;
        let frame_h = frame.area().height;

        match (def, state) {
            (
                FieldDef::Dropdown { options, .. },
                FieldState::Dropdown {
                    selected,
                    open: true,
                    scroll,
                },
            ) => {
                let visible = (options.len()).min(POPUP_VISIBLE_ITEMS);
                let popup_height = (visible as u16 + 2).min(frame_h.saturating_sub(field_y));
                if popup_height < 3 {
                    return; // Not enough space for popup
                }
                let popup_y = field_y.saturating_add(1).min(
                    frame_h.saturating_sub(popup_height),
                );
                let popup_width = available_width.min(40).min(frame_w.saturating_sub(popup_x));
                if popup_width < 4 {
                    return;
                }
                let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

                frame.render_widget(Clear, popup_area);

                let lines: Vec<Line> = options
                    .iter()
                    .enumerate()
                    .skip(*scroll)
                    .take(visible)
                    .map(|(i, opt)| {
                        let is_sel = *selected == Some(i);
                        let prefix = if is_sel { "▸ " } else { "  " };
                        let style = if is_sel {
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        };
                        Line::from(Span::styled(format!("{prefix}{}", opt.display), style))
                    })
                    .collect();

                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan));
                let widget = Paragraph::new(lines).block(block);
                frame.render_widget(widget, popup_area);
            }
            (
                FieldDef::MultiSelect { options, .. },
                FieldState::MultiSelect {
                    selected,
                    open: true,
                    scroll,
                },
            ) => {
                let visible = (options.len()).min(POPUP_VISIBLE_ITEMS);
                let popup_height = (visible as u16 + 2).min(frame_h.saturating_sub(field_y));
                if popup_height < 3 {
                    return;
                }
                let popup_y = field_y.saturating_add(1).min(
                    frame_h.saturating_sub(popup_height),
                );
                let popup_width = available_width.min(40).min(frame_w.saturating_sub(popup_x));
                if popup_width < 4 {
                    return;
                }
                let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

                frame.render_widget(Clear, popup_area);

                let cursor = *scroll; // scroll doubles as cursor in MultiSelect
                let view_start = if cursor >= visible {
                    cursor - visible + 1
                } else {
                    0
                };
                let lines: Vec<Line> = options
                    .iter()
                    .enumerate()
                    .skip(view_start)
                    .take(visible)
                    .map(|(i, opt)| {
                        let is_cursor = i == cursor;
                        let checked = selected.get(i).copied().unwrap_or(false);
                        let check = if checked { "[x]" } else { "[ ]" };
                        let prefix = if is_cursor { "▸" } else { " " };
                        let style = if is_cursor {
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        };
                        Line::from(Span::styled(
                            format!("{prefix} {check} {}", opt.display),
                            style,
                        ))
                    })
                    .collect();

                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan));
                let widget = Paragraph::new(lines).block(block);
                frame.render_widget(widget, popup_area);
            }
            _ => {}
        }
    }

    fn render_confirm_view(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(format!(" Confirm {} ", self.title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let label_width = self.label_width(inner.width);

        let mut y = inner.y;

        for (def, state) in &self.fields {
            if y >= inner.y + inner.height.saturating_sub(1) {
                break;
            }
            let label_text = format!("{:>width$}: ", def.label(), width = label_width);
            let value_text = self.confirm_value_text(def, state);
            let line = Line::from(vec![
                Span::styled(label_text, Style::default().fg(Color::Cyan)),
                Span::styled(value_text, Style::default().fg(Color::White)),
            ]);
            let line_area = Rect::new(inner.x, y, inner.width, 1);
            frame.render_widget(Paragraph::new(line), line_area);
            y += 1;
        }

        // Confirm hint at bottom
        let hint_y = (inner.y + inner.height).saturating_sub(1);
        if hint_y > y {
            let hint = Line::from(Span::styled(
                " Enter Confirm  Esc Back ",
                Style::default().fg(Color::DarkGray),
            ));
            let hint_area = Rect::new(inner.x, hint_y, inner.width, 1);
            frame.render_widget(Paragraph::new(hint), hint_area);
        }
    }

    fn confirm_value_text(&self, def: &FieldDef, state: &FieldState) -> String {
        match (def, state) {
            (FieldDef::Text { password, .. }, FieldState::Text { value, .. }) => {
                if value.is_empty() {
                    "(empty)".into()
                } else if *password {
                    "*".repeat(value.chars().count())
                } else {
                    value.clone()
                }
            }
            (FieldDef::Dropdown { options, .. }, FieldState::Dropdown { selected, .. }) => {
                selected
                    .and_then(|i| options.get(i))
                    .map(|o| o.display.clone())
                    .unwrap_or_else(|| "(none)".into())
            }
            (FieldDef::MultiSelect { options, .. }, FieldState::MultiSelect { selected, .. }) => {
                let vals: Vec<&str> = options
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| selected.get(*i).copied().unwrap_or(false))
                    .map(|(_, o)| o.display.as_str())
                    .collect();
                if vals.is_empty() { "(none)".into() } else { vals.join(", ") }
            }
            (FieldDef::Checkbox { .. }, FieldState::Checkbox { checked }) => {
                if *checked { "Yes".into() } else { "No".into() }
            }
            _ => String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a char-based cursor position to a byte offset in the string.
fn char_to_byte_pos(s: &str, char_pos: usize) -> usize {
    s.char_indices()
        .nth(char_pos)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

fn is_valid_ipv4_cidr(s: &str) -> bool {
    let Some((ip_part, prefix_part)) = s.split_once('/') else {
        return false;
    };
    let Ok(prefix) = prefix_part.parse::<u8>() else {
        return false;
    };
    if prefix > 32 {
        return false;
    }
    let octets: Vec<&str> = ip_part.split('.').collect();
    if octets.len() != 4 {
        return false;
    }
    octets.iter().all(|o| o.parse::<u8>().is_ok())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    // -- Type construction --------------------------------------------------

    #[test]
    fn test_select_option_new() {
        let opt = SelectOption::new("id-1", "Display 1");
        assert_eq!(opt.value, "id-1");
        assert_eq!(opt.display, "Display 1");
    }

    #[test]
    fn test_select_option_simple() {
        let opt = SelectOption::simple("same");
        assert_eq!(opt.value, "same");
        assert_eq!(opt.display, "same");
    }

    #[test]
    fn test_field_def_text_builder() {
        let def = FieldDef::text("Name", true);
        assert_eq!(def.name(), "Name");
        assert_eq!(def.label(), "Name");
        assert!(def.validations().contains(&Validation::Required));
    }

    #[test]
    fn test_field_def_dropdown_builder() {
        let def = FieldDef::dropdown("Flavor", vec!["m1.small".into(), "m1.large".into()], true);
        assert_eq!(def.name(), "Flavor");
        if let FieldDef::Dropdown { options, .. } = &def {
            assert_eq!(options.len(), 2);
            assert_eq!(options[0].value, "m1.small");
        } else {
            panic!("Expected Dropdown");
        }
    }

    #[test]
    fn test_field_def_checkbox_builder() {
        let def = FieldDef::checkbox("Public");
        assert_eq!(def.name(), "Public");
        assert!(def.validations().is_empty());
    }

    #[test]
    fn test_field_def_multiselect_builder() {
        let def = FieldDef::multiselect(
            "Networks",
            vec![SelectOption::new("net1", "Network 1"), SelectOption::new("net2", "Network 2")],
        );
        assert_eq!(def.name(), "Networks");
        if let FieldDef::MultiSelect { options, .. } = &def {
            assert_eq!(options.len(), 2);
        } else {
            panic!("Expected MultiSelect");
        }
    }

    #[test]
    fn test_field_state_default_for() {
        let text_state = FieldState::default_for(&FieldDef::text("Name", false));
        assert!(matches!(text_state, FieldState::Text { .. }));

        let dd_state = FieldState::default_for(&FieldDef::dropdown("X", vec!["a".into()], false));
        assert!(matches!(dd_state, FieldState::Dropdown { selected: None, open: false, .. }));

        let ms_def = FieldDef::multiselect("X", vec![SelectOption::simple("a"), SelectOption::simple("b")]);
        let ms_state = FieldState::default_for(&ms_def);
        if let FieldState::MultiSelect { selected, .. } = ms_state {
            assert_eq!(selected.len(), 2);
            assert!(!selected[0]);
        } else {
            panic!("Expected MultiSelect state");
        }

        let cb_state = FieldState::default_for(&FieldDef::checkbox("X"));
        assert!(matches!(cb_state, FieldState::Checkbox { checked: false }));
    }

    // -- Navigation ---------------------------------------------------------

    #[test]
    fn test_field_navigation_down_up() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::text("A", false),
            FieldDef::text("B", false),
            FieldDef::text("C", false),
        ]);
        assert_eq!(form.focused_index(), 0);

        form.handle_key(key(KeyCode::Down));
        assert_eq!(form.focused_index(), 1);

        form.handle_key(key(KeyCode::Down));
        assert_eq!(form.focused_index(), 2);

        // Clamp at max
        form.handle_key(key(KeyCode::Down));
        assert_eq!(form.focused_index(), 2);

        form.handle_key(key(KeyCode::Up));
        assert_eq!(form.focused_index(), 1);

        form.handle_key(key(KeyCode::Up));
        assert_eq!(form.focused_index(), 0);

        // Clamp at 0
        form.handle_key(key(KeyCode::Up));
        assert_eq!(form.focused_index(), 0);
    }

    #[test]
    fn test_tab_navigation() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::text("A", false),
            FieldDef::text("B", false),
        ]);
        form.handle_key(key(KeyCode::Tab));
        assert_eq!(form.focused_index(), 1);

        form.handle_key(key(KeyCode::BackTab));
        assert_eq!(form.focused_index(), 0);
    }

    #[test]
    fn test_focused_field_name() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::text("Name", false),
            FieldDef::text("Size", false),
        ]);
        assert_eq!(form.focused_field_name(), "Name");
        form.handle_key(key(KeyCode::Down));
        assert_eq!(form.focused_field_name(), "Size");
    }

    // -- Text input ---------------------------------------------------------

    #[test]
    fn test_text_input_and_backspace() {
        let mut form = FormWidget::new("Test", vec![FieldDef::text("Name", false)]);
        form.handle_key(key(KeyCode::Char('a')));
        form.handle_key(key(KeyCode::Char('b')));
        form.handle_key(key(KeyCode::Char('c')));

        if let (_, FieldState::Text { value, cursor }) = &form.fields()[0] {
            assert_eq!(value, "abc");
            assert_eq!(*cursor, 3);
        } else {
            panic!("Expected Text state");
        }

        form.handle_key(key(KeyCode::Backspace));
        if let (_, FieldState::Text { value, .. }) = &form.fields()[0] {
            assert_eq!(value, "ab");
        }
    }

    #[test]
    fn test_text_space_input() {
        let mut form = FormWidget::new("Test", vec![FieldDef::text("Name", false)]);
        form.handle_key(key(KeyCode::Char('a')));
        form.handle_key(key(KeyCode::Char(' ')));
        form.handle_key(key(KeyCode::Char('b')));

        if let (_, FieldState::Text { value, .. }) = &form.fields()[0] {
            assert_eq!(value, "a b");
        }
    }

    #[test]
    fn test_text_max_length_clamp() {
        let mut form = FormWidget::new("Test", vec![FieldDef::text("Name", false)]);
        for _ in 0..MAX_INPUT_LEN + 10 {
            form.handle_key(key(KeyCode::Char('x')));
        }
        if let (_, FieldState::Text { value, .. }) = &form.fields()[0] {
            assert_eq!(value.len(), MAX_INPUT_LEN);
        }
    }

    #[test]
    fn test_char_ignored_on_dropdown() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::dropdown("Type", vec!["a".into(), "b".into()], false),
        ]);
        form.handle_key(key(KeyCode::Char('x')));
        // No crash, no text state change
        assert!(matches!(form.fields()[0].1, FieldState::Dropdown { .. }));
    }

    // -- Dropdown -----------------------------------------------------------

    #[test]
    fn test_dropdown_open_close() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::dropdown("Type", vec!["a".into(), "b".into()], false),
        ]);

        // Open with Enter
        form.handle_key(key(KeyCode::Enter));
        if let FieldState::Dropdown { open, .. } = &form.fields()[0].1 {
            assert!(*open);
        }

        // Close with Esc
        form.handle_key(key(KeyCode::Esc));
        if let FieldState::Dropdown { open, .. } = &form.fields()[0].1 {
            assert!(!*open);
        }

        // Open with Right
        form.handle_key(key(KeyCode::Right));
        if let FieldState::Dropdown { open, .. } = &form.fields()[0].1 {
            assert!(*open);
        }

        // Close with Left (should NOT cancel form, just close popup)
        let action = form.handle_key(key(KeyCode::Left));
        if let FieldState::Dropdown { open, .. } = &form.fields()[0].1 {
            assert!(!*open);
        }
        assert!(matches!(action, FormAction::None));
    }

    #[test]
    fn test_dropdown_navigate_and_select() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::dropdown("Type", vec!["alpha".into(), "beta".into(), "gamma".into()], false),
        ]);

        // Open
        form.handle_key(key(KeyCode::Enter));

        // First Down selects index 0
        form.handle_key(key(KeyCode::Down));
        if let FieldState::Dropdown { selected, .. } = &form.fields()[0].1 {
            assert_eq!(*selected, Some(0));
        }

        // Second Down selects index 1
        form.handle_key(key(KeyCode::Down));
        if let FieldState::Dropdown { selected, .. } = &form.fields()[0].1 {
            assert_eq!(*selected, Some(1));
        }

        // Select with Enter (closes popup)
        form.handle_key(key(KeyCode::Enter));
        if let FieldState::Dropdown { selected, open, .. } = &form.fields()[0].1 {
            assert_eq!(*selected, Some(1));
            assert!(!*open);
        }
    }

    #[test]
    fn test_dropdown_navigate_clamp() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::dropdown("Type", vec!["a".into(), "b".into()], false),
        ]);
        form.handle_key(key(KeyCode::Enter)); // open

        // Up at top stays at 0
        form.handle_key(key(KeyCode::Up));
        if let FieldState::Dropdown { selected, .. } = &form.fields()[0].1 {
            assert_eq!(*selected, Some(0));
        }

        // Down to max
        form.handle_key(key(KeyCode::Down));
        form.handle_key(key(KeyCode::Down));
        if let FieldState::Dropdown { selected, .. } = &form.fields()[0].1 {
            assert_eq!(*selected, Some(1)); // clamped at max
        }
    }

    // -- MultiSelect --------------------------------------------------------

    #[test]
    fn test_multiselect_toggle() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::multiselect("Nets", vec![
                SelectOption::simple("net1"),
                SelectOption::simple("net2"),
                SelectOption::simple("net3"),
            ]),
        ]);

        // Open
        form.handle_key(key(KeyCode::Enter));

        // Toggle first item
        form.handle_key(key(KeyCode::Char(' ')));
        if let FieldState::MultiSelect { selected, .. } = &form.fields()[0].1 {
            assert!(selected[0]);
            assert!(!selected[1]);
        }

        // Move down and toggle
        form.handle_key(key(KeyCode::Down));
        form.handle_key(key(KeyCode::Char(' ')));
        if let FieldState::MultiSelect { selected, .. } = &form.fields()[0].1 {
            assert!(selected[0]);
            assert!(selected[1]);
            assert!(!selected[2]);
        }

        // Untoggle first
        form.handle_key(key(KeyCode::Up));
        form.handle_key(key(KeyCode::Char(' ')));
        if let FieldState::MultiSelect { selected, .. } = &form.fields()[0].1 {
            assert!(!selected[0]);
            assert!(selected[1]);
        }
    }

    #[test]
    fn test_multiselect_close() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::multiselect("Nets", vec![SelectOption::simple("net1")]),
        ]);
        form.handle_key(key(KeyCode::Enter)); // open
        let action = form.handle_key(key(KeyCode::Esc)); // close
        if let FieldState::MultiSelect { open, .. } = &form.fields()[0].1 {
            assert!(!*open);
        }
        assert!(matches!(action, FormAction::None));
    }

    // -- Checkbox -----------------------------------------------------------

    #[test]
    fn test_checkbox_toggle_enter() {
        let mut form = FormWidget::new("Test", vec![FieldDef::checkbox("Public")]);
        form.handle_key(key(KeyCode::Enter));
        if let FieldState::Checkbox { checked } = &form.fields()[0].1 {
            assert!(*checked);
        }
        form.handle_key(key(KeyCode::Enter));
        if let FieldState::Checkbox { checked } = &form.fields()[0].1 {
            assert!(!*checked);
        }
    }

    #[test]
    fn test_checkbox_toggle_space() {
        let mut form = FormWidget::new("Test", vec![FieldDef::checkbox("Public")]);
        form.handle_key(key(KeyCode::Char(' ')));
        if let FieldState::Checkbox { checked } = &form.fields()[0].1 {
            assert!(*checked);
        }
    }

    #[test]
    fn test_checkbox_toggle_right() {
        let mut form = FormWidget::new("Test", vec![FieldDef::checkbox("Public")]);
        form.handle_key(key(KeyCode::Right));
        if let FieldState::Checkbox { checked } = &form.fields()[0].1 {
            assert!(*checked);
        }
    }

    // -- Validation ---------------------------------------------------------

    #[test]
    fn test_validate_required_text() {
        let mut form = FormWidget::new("Test", vec![FieldDef::text("Name", true)]);
        let action = form.validate_and_submit();
        assert!(matches!(action, FormAction::None));
        assert_eq!(form.errors().len(), 1);
        assert!(form.errors()[0].message.contains("required"));
    }

    #[test]
    fn test_validate_required_dropdown() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::dropdown("Flavor", vec!["m1.small".into()], true),
        ]);
        let action = form.validate_and_submit();
        assert!(matches!(action, FormAction::None));
        assert_eq!(form.errors().len(), 1);
        assert!(form.errors()[0].message.contains("required"));
    }

    #[test]
    fn test_validate_min_length() {
        let def = FieldDef::Text {
            name: "Pass".into(),
            label: "Password".into(),
            placeholder: String::new(),
            validations: vec![Validation::MinLength(4)],
            password: true,
        };
        let mut form = FormWidget::new("Test", vec![def]);
        form.handle_key(key(KeyCode::Char('a')));
        form.handle_key(key(KeyCode::Char('b')));
        let action = form.validate_and_submit();
        assert!(matches!(action, FormAction::None));
        assert!(form.errors()[0].message.contains("at least 4"));
    }

    #[test]
    fn test_validate_numeric() {
        let def = FieldDef::Text {
            name: "Port".into(),
            label: "Port".into(),
            placeholder: String::new(),
            validations: vec![Validation::Numeric],
            password: false,
        };
        let mut form = FormWidget::new("Test", vec![def]);
        form.handle_key(key(KeyCode::Char('a')));
        let action = form.validate_and_submit();
        assert!(matches!(action, FormAction::None));
        assert!(form.errors()[0].message.contains("numeric"));
    }

    #[test]
    fn test_validate_cidr_valid() {
        let def = FieldDef::Text {
            name: "CIDR".into(),
            label: "CIDR".into(),
            placeholder: String::new(),
            validations: vec![Validation::Cidr],
            password: false,
        };
        let mut form = FormWidget::new("Test", vec![def]);
        // Type "10.0.0.0/24"
        for c in "10.0.0.0/24".chars() {
            form.handle_key(key(KeyCode::Char(c)));
        }
        form.validate_and_submit();
        assert_eq!(form.phase(), FormPhase::Confirming);
        let action = form.confirm_submit();
        assert!(matches!(action, FormAction::Submit(_)));
    }

    #[test]
    fn test_validate_cidr_invalid() {
        let def = FieldDef::Text {
            name: "CIDR".into(),
            label: "CIDR".into(),
            placeholder: String::new(),
            validations: vec![Validation::Cidr],
            password: false,
        };
        let mut form = FormWidget::new("Test", vec![def]);
        for c in "not-a-cidr".chars() {
            form.handle_key(key(KeyCode::Char(c)));
        }
        let action = form.validate_and_submit();
        assert!(matches!(action, FormAction::None));
        assert!(form.errors()[0].message.contains("CIDR"));
    }

    #[test]
    fn test_validate_focuses_first_error() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::text("A", false),
            FieldDef::text("B", true),
            FieldDef::text("C", true),
        ]);
        form.handle_key(key(KeyCode::Down)); // focus B
        form.handle_key(key(KeyCode::Down)); // focus C
        // A is not required, B and C are required (empty)
        form.validate_and_submit();
        assert_eq!(form.focused_index(), 1); // Focus moved to B (first error)
    }

    // -- Submit / Cancel ----------------------------------------------------

    #[test]
    fn test_submit_with_valid_data() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::text("Name", true),
            FieldDef::checkbox("Public"),
        ]);
        form.handle_key(key(KeyCode::Char('x')));
        form.validate_and_submit();
        let action = form.confirm_submit();
        if let FormAction::Submit(values) = action {
            assert_eq!(values.get("Name"), Some(&FormValue::Text("x".into())));
            assert_eq!(values.get("Public"), Some(&FormValue::Bool(false)));
        } else {
            panic!("Expected Submit");
        }
    }

    #[test]
    fn test_submit_dropdown_value() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::dropdown("Flavor", vec!["small".into(), "large".into()], false),
        ]);
        // Open, Down twice (0 → 1 = "large"), close
        form.handle_key(key(KeyCode::Enter));
        form.handle_key(key(KeyCode::Down)); // selects index 0 ("small")
        form.handle_key(key(KeyCode::Down)); // selects index 1 ("large")
        form.handle_key(key(KeyCode::Right)); // close without submit

        form.validate_and_submit();
        let action = form.confirm_submit();
        if let FormAction::Submit(values) = action {
            assert_eq!(values.get("Flavor"), Some(&FormValue::Selected("large".into())));
        } else {
            panic!("Expected Submit");
        }
    }

    #[test]
    fn test_submit_multiselect_values() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::multiselect("Nets", vec![
                SelectOption::new("n1", "Net 1"),
                SelectOption::new("n2", "Net 2"),
                SelectOption::new("n3", "Net 3"),
            ]),
        ]);
        // Open, toggle first, down, toggle second, close
        form.handle_key(key(KeyCode::Enter));
        form.handle_key(key(KeyCode::Char(' ')));
        form.handle_key(key(KeyCode::Down));
        form.handle_key(key(KeyCode::Char(' ')));
        form.handle_key(key(KeyCode::Enter));

        form.validate_and_submit();
        let action = form.confirm_submit();
        if let FormAction::Submit(values) = action {
            assert_eq!(
                values.get("Nets"),
                Some(&FormValue::MultiSelected(vec!["n1".into(), "n2".into()]))
            );
        } else {
            panic!("Expected Submit");
        }
    }

    #[test]
    fn test_cancel_with_left() {
        let mut form = FormWidget::new("Test", vec![FieldDef::text("Name", false)]);
        let action = form.handle_key(key(KeyCode::Left));
        assert!(matches!(action, FormAction::Cancel));
    }

    #[test]
    fn test_cancel_with_esc() {
        let mut form = FormWidget::new("Test", vec![FieldDef::text("Name", false)]);
        let action = form.handle_key(key(KeyCode::Esc));
        assert!(matches!(action, FormAction::Cancel));
    }

    #[test]
    fn test_esc_in_dropdown_closes_not_cancels() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::dropdown("Type", vec!["a".into()], false),
        ]);
        form.handle_key(key(KeyCode::Enter)); // open
        let action = form.handle_key(key(KeyCode::Esc)); // close popup, not cancel form
        assert!(matches!(action, FormAction::None));
        if let FieldState::Dropdown { open, .. } = &form.fields()[0].1 {
            assert!(!*open);
        }
    }

    #[test]
    fn test_left_in_dropdown_closes_not_cancels() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::dropdown("Type", vec!["a".into()], false),
        ]);
        form.handle_key(key(KeyCode::Right)); // open
        let action = form.handle_key(key(KeyCode::Left)); // close popup
        assert!(matches!(action, FormAction::None));
    }

    #[test]
    fn test_enter_on_last_field_enters_confirming() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::text("Name", false),
        ]);
        let action = form.handle_key(key(KeyCode::Enter));
        assert!(matches!(action, FormAction::None));
        assert_eq!(form.phase(), FormPhase::Confirming);
    }

    // -- set_field_options / set_field_value ---------------------------------

    #[test]
    fn test_set_field_options() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::dropdown("Flavor", vec![], false),
        ]);

        form.set_field_options("Flavor", vec![
            SelectOption::new("s1", "Small"),
            SelectOption::new("l1", "Large"),
        ]);

        if let (FieldDef::Dropdown { options, .. }, _) = &form.fields()[0] {
            assert_eq!(options.len(), 2);
            assert_eq!(options[0].value, "s1");
        } else {
            panic!("Expected Dropdown");
        }
    }

    #[test]
    fn test_set_field_value_text() {
        let mut form = FormWidget::new("Test", vec![FieldDef::text("Name", false)]);
        form.set_field_value("Name", FormValue::Text("preset".into()));
        if let (_, FieldState::Text { value, cursor }) = &form.fields()[0] {
            assert_eq!(value, "preset");
            assert_eq!(*cursor, 6);
        }
    }

    #[test]
    fn test_set_field_value_dropdown() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::dropdown("Size", vec!["small".into(), "large".into()], false),
        ]);
        form.set_field_value("Size", FormValue::Selected("large".into()));
        if let (_, FieldState::Dropdown { selected, .. }) = &form.fields()[0] {
            assert_eq!(*selected, Some(1));
        }
    }

    #[test]
    fn test_set_field_value_checkbox() {
        let mut form = FormWidget::new("Test", vec![FieldDef::checkbox("Public")]);
        form.set_field_value("Public", FormValue::Bool(true));
        if let (_, FieldState::Checkbox { checked }) = &form.fields()[0] {
            assert!(*checked);
        }
    }

    #[test]
    fn test_set_field_value_multiselect() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::multiselect("Nets", vec![
                SelectOption::new("n1", "Net 1"),
                SelectOption::new("n2", "Net 2"),
                SelectOption::new("n3", "Net 3"),
            ]),
        ]);
        form.set_field_value("Nets", FormValue::MultiSelected(vec!["n1".into(), "n3".into()]));
        if let (_, FieldState::MultiSelect { selected, .. }) = &form.fields()[0] {
            assert!(selected[0]);
            assert!(!selected[1]);
            assert!(selected[2]);
        }
    }

    // -- Review fix: Enter on non-last field does NOT submit -----------------

    #[test]
    fn test_enter_on_middle_text_field_does_not_submit() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::text("Name", false),
            FieldDef::text("Size", false),
            FieldDef::text("Zone", false),
        ]);
        // Focus is on field 0 (not last)
        let action = form.handle_key(key(KeyCode::Enter));
        assert!(matches!(action, FormAction::None));
    }

    // -- Review fix: Dropdown scroll tracks down direction -------------------

    #[test]
    fn test_dropdown_scroll_tracks_down() {
        let opts: Vec<String> = (0..20).map(|i| format!("opt-{i}")).collect();
        let mut form = FormWidget::new("Test", vec![
            FieldDef::dropdown("Big", opts, false),
        ]);
        form.handle_key(key(KeyCode::Enter)); // open

        // Navigate down past POPUP_VISIBLE_ITEMS (first Down = index 0)
        for _ in 0..15 {
            form.handle_key(key(KeyCode::Down));
        }
        if let FieldState::Dropdown { selected, scroll, .. } = &form.fields()[0].1 {
            assert_eq!(*selected, Some(14)); // 15 Downs from None: 0..14
            assert!(*scroll > 0, "scroll should have advanced but was {scroll}");
        }
    }

    // -- Review fix: MultiSelect Required validation -------------------------

    #[test]
    fn test_validate_required_multiselect_empty() {
        let def = FieldDef::MultiSelect {
            name: "Nets".into(),
            label: "Networks".into(),
            validations: vec![Validation::Required],
            options: vec![SelectOption::simple("net1"), SelectOption::simple("net2")],
        };
        let mut form = FormWidget::new("Test", vec![def]);
        let action = form.validate_and_submit();
        assert!(matches!(action, FormAction::None));
        assert_eq!(form.errors().len(), 1);
        assert!(form.errors()[0].message.contains("required"));
    }

    #[test]
    fn test_validate_required_multiselect_with_selection() {
        let def = FieldDef::MultiSelect {
            name: "Nets".into(),
            label: "Networks".into(),
            validations: vec![Validation::Required],
            options: vec![SelectOption::simple("net1"), SelectOption::simple("net2")],
        };
        let mut form = FormWidget::new("Test", vec![def]);
        // Open, toggle first, close
        form.handle_key(key(KeyCode::Enter));
        form.handle_key(key(KeyCode::Char(' ')));
        form.handle_key(key(KeyCode::Enter));

        form.validate_and_submit();
        let action = form.confirm_submit();
        assert!(matches!(action, FormAction::Submit(_)));
    }

    // -- Review fix: set_field_value respects MAX_INPUT_LEN ------------------

    #[test]
    fn test_set_field_value_text_truncates() {
        let mut form = FormWidget::new("Test", vec![FieldDef::text("Name", false)]);
        let long_str = "x".repeat(MAX_INPUT_LEN + 100);
        form.set_field_value("Name", FormValue::Text(long_str));
        if let (_, FieldState::Text { value, cursor }) = &form.fields()[0] {
            assert_eq!(value.chars().count(), MAX_INPUT_LEN);
            assert_eq!(*cursor, MAX_INPUT_LEN);
        }
    }

    // -- Council fix #1: UTF-8 multi-byte safety ----------------------------

    #[test]
    fn test_utf8_text_input_and_backspace() {
        let mut form = FormWidget::new("Test", vec![FieldDef::text("Name", false)]);
        // Type Korean characters
        form.handle_key(key(KeyCode::Char('한')));
        form.handle_key(key(KeyCode::Char('글')));
        form.handle_key(key(KeyCode::Char('!')));

        if let (_, FieldState::Text { value, cursor }) = &form.fields()[0] {
            assert_eq!(value, "한글!");
            assert_eq!(*cursor, 3); // 3 chars, not byte count
        }

        // Backspace removes last char correctly
        form.handle_key(key(KeyCode::Backspace));
        if let (_, FieldState::Text { value, cursor }) = &form.fields()[0] {
            assert_eq!(value, "한글");
            assert_eq!(*cursor, 2);
        }

        form.handle_key(key(KeyCode::Backspace));
        if let (_, FieldState::Text { value, cursor }) = &form.fields()[0] {
            assert_eq!(value, "한");
            assert_eq!(*cursor, 1);
        }
    }

    #[test]
    fn test_utf8_set_field_value() {
        let mut form = FormWidget::new("Test", vec![FieldDef::text("Name", false)]);
        form.set_field_value("Name", FormValue::Text("안녕하세요".into()));
        if let (_, FieldState::Text { value, cursor }) = &form.fields()[0] {
            assert_eq!(value, "안녕하세요");
            assert_eq!(*cursor, 5); // 5 chars
        }
    }

    // -- Council fix #2: Last field Dropdown/Checkbox submit ----------------

    #[test]
    fn test_submit_when_last_field_is_dropdown() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::text("Name", false),
            FieldDef::dropdown("Type", vec!["a".into(), "b".into()], false),
        ]);
        // Type name
        form.handle_key(key(KeyCode::Char('x')));
        // Move to last field (Dropdown)
        form.handle_key(key(KeyCode::Down));
        // Open dropdown
        form.handle_key(key(KeyCode::Enter));
        // Select first option and close with Enter — enters confirming
        let action = form.handle_key(key(KeyCode::Enter));
        assert!(matches!(action, FormAction::None));
        assert_eq!(form.phase(), FormPhase::Confirming);
    }

    #[test]
    fn test_submit_when_last_field_is_checkbox() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::checkbox("Accept"),
        ]);
        // Toggle checkbox (Enter toggles but doesn't submit for checkbox)
        form.handle_key(key(KeyCode::Enter));
        // but we need a way to submit. Use Tab to stay, then test validate_and_submit directly.
        form.validate_and_submit();
        let action = form.confirm_submit();
        assert!(matches!(action, FormAction::Submit(_)));
    }

    // -- Council fix #3: Dropdown first Down starts at index 0 --------------

    #[test]
    fn test_dropdown_first_down_selects_index_0() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::dropdown("Type", vec!["alpha".into(), "beta".into(), "gamma".into()], false),
        ]);
        form.handle_key(key(KeyCode::Enter)); // open
        form.handle_key(key(KeyCode::Down)); // first down
        if let FieldState::Dropdown { selected, .. } = &form.fields()[0].1 {
            assert_eq!(*selected, Some(0)); // should be 0, not 1
        }
    }

    // -- Council fix #4: MinLength/MaxLength uses char count ----------------

    #[test]
    fn test_min_length_counts_chars_not_bytes() {
        let def = FieldDef::Text {
            name: "Name".into(),
            label: "Name".into(),
            placeholder: String::new(),
            validations: vec![Validation::MinLength(3)],
            password: false,
        };
        let mut form = FormWidget::new("Test", vec![def]);
        // Type 3 Korean chars (9 bytes but 3 chars)
        form.handle_key(key(KeyCode::Char('가')));
        form.handle_key(key(KeyCode::Char('나')));
        form.handle_key(key(KeyCode::Char('다')));
        form.validate_and_submit();
        let action = form.confirm_submit();
        assert!(matches!(action, FormAction::Submit(_)));
    }

    // -- Council fix #5: Duplicate field name detection --------------------

    #[test]
    #[should_panic(expected = "unique")]
    fn test_duplicate_field_names_panics_in_debug() {
        let _form = FormWidget::new("Test", vec![
            FieldDef::text("Name", false),
            FieldDef::text("Name", false),
        ]);
    }

    // -- CIDR helper --------------------------------------------------------

    #[test]
    fn test_cidr_validation_helper() {
        assert!(is_valid_ipv4_cidr("10.0.0.0/24"));
        assert!(is_valid_ipv4_cidr("192.168.1.0/16"));
        assert!(is_valid_ipv4_cidr("0.0.0.0/0"));
        assert!(!is_valid_ipv4_cidr("10.0.0.0"));       // no prefix
        assert!(!is_valid_ipv4_cidr("10.0.0.0/33"));     // prefix > 32
        assert!(!is_valid_ipv4_cidr("10.0.0/24"));       // only 3 octets
        assert!(!is_valid_ipv4_cidr("abc.0.0.0/24"));    // non-numeric
    }

    // -- Render tests -------------------------------------------------------

    fn render_to_buffer(form: &FormWidget, width: u16, height: u16) -> String {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = Rect::new(0, 0, width, height);
                form.render(f, area);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut output = String::new();
        for y in 0..height {
            for x in 0..width {
                let cell = &buf[(x, y)];
                output.push_str(cell.symbol());
            }
            output.push('\n');
        }
        output
    }

    #[test]
    fn test_render_shows_title() {
        let form = FormWidget::new("Create Server", vec![
            FieldDef::text("Name", true),
        ]);
        let output = render_to_buffer(&form, 50, 10);
        assert!(output.contains("Create Server"), "Title not found in render output");
    }

    #[test]
    fn test_render_shows_field_labels() {
        let form = FormWidget::new("Test", vec![
            FieldDef::text("Name", false),
            FieldDef::dropdown("Flavor", vec!["m1.small".into()], false),
            FieldDef::checkbox("Public"),
        ]);
        let output = render_to_buffer(&form, 60, 12);
        assert!(output.contains("Name"), "Name label not found");
        assert!(output.contains("Flavor"), "Flavor label not found");
        assert!(output.contains("Public"), "Public label not found");
    }

    #[test]
    fn test_render_shows_checkbox_state() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::checkbox("Accept"),
        ]);
        let output = render_to_buffer(&form, 40, 8);
        assert!(output.contains("[ ]"), "Unchecked state not found");

        form.handle_key(key(KeyCode::Char(' ')));
        let output = render_to_buffer(&form, 40, 8);
        assert!(output.contains("[x]"), "Checked state not found");
    }

    #[test]
    fn test_render_shows_dropdown_arrow() {
        let form = FormWidget::new("Test", vec![
            FieldDef::dropdown("Type", vec!["a".into()], false),
        ]);
        let output = render_to_buffer(&form, 40, 8);
        assert!(
            output.contains('▼') || output.contains("(select)"),
            "Dropdown indicator not found"
        );
    }

    #[test]
    fn test_render_shows_validation_error() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::text("Name", true),
        ]);
        form.validate_and_submit();
        let output = render_to_buffer(&form, 50, 10);
        assert!(output.contains("required"), "Validation error not shown");
    }

    #[test]
    fn test_render_no_panic_on_small_area() {
        let form = FormWidget::new("Test", vec![
            FieldDef::text("Name", false),
            FieldDef::text("Size", false),
        ]);
        // Should not panic even with tiny area
        let _output = render_to_buffer(&form, 5, 3);
    }

    #[test]
    fn test_render_popup_on_small_area_no_panic() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::dropdown("Type", vec!["a".into(), "b".into(), "c".into()], false),
        ]);
        // Open popup
        form.handle_key(key(KeyCode::Enter));
        // Render on very small area — must not panic from u16 underflow
        let _output = render_to_buffer(&form, 10, 4);
    }

    #[test]
    fn test_render_popup_with_errors_above() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::text("Name", true),
            FieldDef::dropdown("Type", vec!["a".into(), "b".into()], false),
        ]);
        // Trigger validation error on Name
        form.validate_and_submit();
        // Move to dropdown and open popup
        form.handle_key(key(KeyCode::Down));
        form.handle_key(key(KeyCode::Enter));
        // Should render without panic — popup y accounts for error line
        let _output = render_to_buffer(&form, 50, 15);
    }

    #[test]
    fn test_render_shows_text_value() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::text("Name", false),
        ]);
        form.handle_key(key(KeyCode::Char('h')));
        form.handle_key(key(KeyCode::Char('i')));
        let output = render_to_buffer(&form, 40, 8);
        assert!(output.contains('h'), "Text value not rendered");
    }

    #[test]
    fn test_render_required_asterisk() {
        let form = FormWidget::new("Test", vec![
            FieldDef::text("Name", true),
            FieldDef::text("Description", false),
        ]);
        let output = render_to_buffer(&form, 60, 10);
        // Check that Name line has * but Description line doesn't
        let name_line = output.lines().find(|l| l.contains("Name")).expect("Name line not found");
        assert!(name_line.contains("*"), "Required Name field missing * indicator");
        let desc_line = output.lines().find(|l| l.contains("Description")).expect("Desc line not found");
        assert!(!desc_line.contains("*"), "Optional Description should not have *");
    }

    #[test]
    fn test_form_initial_phase_is_editing() {
        let form = FormWidget::new("Test", vec![
            FieldDef::text("Name", true),
        ]);
        assert_eq!(form.phase(), FormPhase::Editing);
    }

    #[test]
    fn test_submit_enters_confirming_phase() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::text("Name", true),
        ]);
        // Type a value so validation passes
        form.handle_key(key(KeyCode::Char('s')));
        form.handle_key(key(KeyCode::Char('v')));
        // Enter on last field triggers submit flow
        let action = form.handle_key(key(KeyCode::Enter));
        // Should NOT submit yet — should enter Confirming phase
        assert!(matches!(action, FormAction::None), "Should not submit directly");
        assert_eq!(form.phase(), FormPhase::Confirming, "Should be in Confirming phase");
    }

    #[test]
    fn test_confirming_enter_submits() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::text("Name", false),
        ]);
        form.handle_key(key(KeyCode::Char('x')));
        form.handle_key(key(KeyCode::Enter)); // enters Confirming
        assert_eq!(form.phase(), FormPhase::Confirming);
        let action = form.handle_key(key(KeyCode::Enter)); // confirms
        assert!(matches!(action, FormAction::Submit(_)));
        if let FormAction::Submit(values) = action {
            assert_eq!(values.get("Name"), Some(&FormValue::Text("x".into())));
        }
    }

    #[test]
    fn test_confirming_esc_returns_to_editing() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::text("Name", false),
        ]);
        form.handle_key(key(KeyCode::Enter)); // enters Confirming
        assert_eq!(form.phase(), FormPhase::Confirming);
        let action = form.handle_key(key(KeyCode::Esc)); // cancel
        assert!(matches!(action, FormAction::None));
        assert_eq!(form.phase(), FormPhase::Editing);
    }

    #[test]
    fn test_render_confirm_view_shows_values() {
        let mut form = FormWidget::new("Create Server", vec![
            FieldDef::text("Name", true),
            FieldDef::text("Zone", false),
        ]);
        form.handle_key(key(KeyCode::Char('w')));
        form.handle_key(key(KeyCode::Char('e')));
        form.handle_key(key(KeyCode::Char('b')));
        // Move to Zone
        form.handle_key(key(KeyCode::Down));
        form.handle_key(key(KeyCode::Char('a')));
        form.handle_key(key(KeyCode::Char('z')));
        // Enter on last field → Confirming
        form.handle_key(key(KeyCode::Enter));
        assert_eq!(form.phase(), FormPhase::Confirming);
        let output = render_to_buffer(&form, 60, 15);
        // Confirm view should show "Confirm" in title
        assert!(output.contains("Confirm"), "Confirm view should show Confirm title");
        // Confirm view should show field values
        assert!(output.contains("web"), "Confirm view should show Name value");
        assert!(output.contains("az"), "Confirm view should show Zone value");
        // Should show confirm-specific hint (different from editing hint)
        assert!(output.contains("Enter Confirm"), "Confirm view should show Enter Confirm hint");
    }

    #[test]
    fn test_confirm_view_masks_password() {
        let mut form = FormWidget::new("Test", vec![
            FieldDef::password("Secret", true),
        ]);
        for c in "abc".chars() {
            form.handle_key(key(KeyCode::Char(c)));
        }
        form.handle_key(key(KeyCode::Enter)); // Confirming
        assert_eq!(form.phase(), FormPhase::Confirming);
        let output = render_to_buffer(&form, 50, 10);
        assert!(output.contains("***"), "Password should be masked in confirm view");
        assert!(!output.contains("abc"), "Password plaintext should not appear");
    }

    #[test]
    fn test_render_hint_shows_required_legend() {
        let form = FormWidget::new("Test", vec![
            FieldDef::text("Name", true),
        ]);
        let output = render_to_buffer(&form, 60, 10);
        assert!(output.contains("* = required"), "Required legend not shown in hint");
    }
}
