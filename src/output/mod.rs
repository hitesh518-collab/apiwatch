use crate::diff::{Change, Severity};

pub fn render_changes(changes: &[Change]) -> String {
    if changes.is_empty() {
        return "No changes detected.\n".to_string();
    }

    let mut rendered = String::new();
    render_group(
        &mut rendered,
        "Breaking changes",
        changes,
        Severity::Breaking,
    );
    render_group(&mut rendered, "Warnings", changes, Severity::Warning);
    render_group(
        &mut rendered,
        "Non-breaking changes",
        changes,
        Severity::NonBreaking,
    );
    rendered
}

fn render_group(rendered: &mut String, title: &str, changes: &[Change], severity: Severity) {
    let group: Vec<_> = changes
        .iter()
        .filter(|change| change.severity == severity)
        .collect();

    if group.is_empty() {
        return;
    }

    if !rendered.is_empty() {
        rendered.push('\n');
    }

    rendered.push_str(title);
    rendered.push('\n');

    for change in group {
        rendered.push_str("- ");
        rendered.push_str(change.operation.method.as_str());
        rendered.push(' ');
        rendered.push_str(&change.operation.path);
        rendered.push_str(": ");
        rendered.push_str(&change.message);
        rendered.push('\n');
    }
}
