use agent_policy_discover::InstructionSourceType;

pub(crate) fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character => escaped.push(character),
        }
    }
    escaped
}

pub(crate) fn markdown_inline(text: &str) -> String {
    text.replace('`', "\\`").replace(['\n', '\r'], " ")
}

pub(crate) fn instruction_source_type_name(source_type: &InstructionSourceType) -> &'static str {
    match source_type {
        InstructionSourceType::AgentsMd => "agents_md",
        InstructionSourceType::ClaudeMd => "claude_md",
        InstructionSourceType::CopilotInstructions => "copilot_instructions",
        InstructionSourceType::CursorRule => "cursor_rule",
    }
}

pub(crate) fn push_unique(items: &mut Vec<String>, item: String) {
    if !items.contains(&item) {
        items.push(item);
    }
}
