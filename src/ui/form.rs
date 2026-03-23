use crossterm::event::{KeyCode, KeyEvent};

const MAX_INPUT_LEN: usize = 256;

#[derive(Debug, Clone)]
pub enum FormFieldType {
    Text,
    Password,
    Dropdown(Vec<String>),
    Checkbox,
}

#[derive(Debug, Clone)]
pub struct FormField {
    pub label: String,
    pub field_type: FormFieldType,
    pub value: String,
    pub required: bool,
    pub selected_option: usize,
    pub checked: bool,
}

impl FormField {
    pub fn text(label: impl Into<String>, required: bool) -> Self {
        Self {
            label: label.into(),
            field_type: FormFieldType::Text,
            value: String::new(),
            required,
            selected_option: 0,
            checked: false,
        }
    }

    pub fn dropdown(label: impl Into<String>, options: Vec<String>, required: bool) -> Self {
        Self {
            label: label.into(),
            field_type: FormFieldType::Dropdown(options),
            value: String::new(),
            required,
            selected_option: 0,
            checked: false,
        }
    }

    pub fn checkbox(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            field_type: FormFieldType::Checkbox,
            value: String::new(),
            required: false,
            selected_option: 0,
            checked: false,
        }
    }
}

pub struct FormWidget {
    fields: Vec<FormField>,
    focused: usize,
}

#[derive(Debug)]
pub enum FormAction {
    Submit(Vec<FormField>),
    Cancel,
    None,
}

impl FormWidget {
    pub fn new(fields: Vec<FormField>) -> Self {
        Self { fields, focused: 0 }
    }

    pub fn focused_index(&self) -> usize {
        self.focused
    }

    pub fn fields(&self) -> &[FormField] {
        &self.fields
    }

    /// Validate: all required fields must have non-empty value.
    pub fn validate(&self) -> Vec<String> {
        self.fields
            .iter()
            .filter(|f| {
                f.required
                    && match &f.field_type {
                        FormFieldType::Text | FormFieldType::Password => f.value.is_empty(),
                        FormFieldType::Dropdown(_) => f.value.is_empty(),
                        FormFieldType::Checkbox => false,
                    }
            })
            .map(|f| format!("{} is required", f.label))
            .collect()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> FormAction {
        match key.code {
            KeyCode::Esc => FormAction::Cancel,
            KeyCode::Tab | KeyCode::Down => {
                if self.focused < self.fields.len().saturating_sub(1) {
                    self.focused += 1;
                }
                FormAction::None
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focused = self.focused.saturating_sub(1);
                FormAction::None
            }
            KeyCode::Enter => {
                let errors = self.validate();
                if errors.is_empty() {
                    FormAction::Submit(self.fields.clone())
                } else {
                    FormAction::None
                }
            }
            KeyCode::Char(' ') => {
                if let Some(field) = self.fields.get_mut(self.focused) {
                    if matches!(field.field_type, FormFieldType::Checkbox) {
                        field.checked = !field.checked;
                    } else {
                        field.value.push(' ');
                    }
                }
                FormAction::None
            }
            KeyCode::Char('j') | KeyCode::Char('l') => {
                // For dropdown: cycle forward. For text: insert char.
                if let Some(field) = self.fields.get_mut(self.focused) {
                    match &field.field_type {
                        FormFieldType::Dropdown(opts) if !opts.is_empty() => {
                            field.selected_option =
                                (field.selected_option + 1) % opts.len();
                            field.value = opts[field.selected_option].clone();
                        }
                        FormFieldType::Text | FormFieldType::Password => {
                            let c = if key.code == KeyCode::Char('j') { 'j' } else { 'l' };
                            if field.value.len() < MAX_INPUT_LEN {
                                field.value.push(c);
                            }
                        }
                        _ => {}
                    }
                }
                FormAction::None
            }
            KeyCode::Char('k') | KeyCode::Char('h') => {
                // For dropdown: cycle backward. For text: insert char.
                if let Some(field) = self.fields.get_mut(self.focused) {
                    match &field.field_type {
                        FormFieldType::Dropdown(opts) if !opts.is_empty() => {
                            field.selected_option = if field.selected_option == 0 {
                                opts.len() - 1
                            } else {
                                field.selected_option - 1
                            };
                            field.value = opts[field.selected_option].clone();
                        }
                        FormFieldType::Text | FormFieldType::Password => {
                            let c = if key.code == KeyCode::Char('k') { 'k' } else { 'h' };
                            if field.value.len() < MAX_INPUT_LEN {
                                field.value.push(c);
                            }
                        }
                        _ => {}
                    }
                }
                FormAction::None
            }
            KeyCode::Char(c) => {
                if let Some(field) = self.fields.get_mut(self.focused) {
                    match &field.field_type {
                        FormFieldType::Text | FormFieldType::Password => {
                            if field.value.len() < MAX_INPUT_LEN {
                                field.value.push(c);
                            }
                        }
                        FormFieldType::Dropdown(_) => {
                            // Ignore direct char input for dropdown; use j/k to cycle
                        }
                        FormFieldType::Checkbox => {}
                    }
                }
                FormAction::None
            }
            KeyCode::Backspace => {
                if let Some(field) = self.fields.get_mut(self.focused) {
                    field.value.pop();
                }
                FormAction::None
            }
            _ => FormAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    #[test]
    fn test_field_navigation() {
        let mut form = FormWidget::new(vec![
            FormField::text("Name", true),
            FormField::text("Size", true),
            FormField::checkbox("Public"),
        ]);
        assert_eq!(form.focused_index(), 0);
        form.handle_key(key(KeyCode::Tab));
        assert_eq!(form.focused_index(), 1);
        form.handle_key(key(KeyCode::Tab));
        assert_eq!(form.focused_index(), 2);
        // At max
        form.handle_key(key(KeyCode::Tab));
        assert_eq!(form.focused_index(), 2);
        form.handle_key(key(KeyCode::Up));
        assert_eq!(form.focused_index(), 1);
    }

    #[test]
    fn test_text_input() {
        let mut form = FormWidget::new(vec![FormField::text("Name", true)]);
        form.handle_key(key(KeyCode::Char('a')));
        form.handle_key(key(KeyCode::Char('b')));
        assert_eq!(form.fields()[0].value, "ab");
        form.handle_key(key(KeyCode::Backspace));
        assert_eq!(form.fields()[0].value, "a");
    }

    #[test]
    fn test_validate_required() {
        let form = FormWidget::new(vec![
            FormField::text("Name", true),
            FormField::text("Desc", false),
        ]);
        let errors = form.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Name"));
    }

    #[test]
    fn test_submit_with_valid_data() {
        let mut form = FormWidget::new(vec![FormField::text("Name", true)]);
        form.handle_key(key(KeyCode::Char('x')));
        let action = form.handle_key(key(KeyCode::Enter));
        assert!(matches!(action, FormAction::Submit(_)));
    }
}
