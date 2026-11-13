pub(crate) fn push_tail_args(
    args: &mut Vec<String>,
    resumed: bool,
    effort: Option<&str>,
    initial_prompt: Option<&str>,
) {
    if let Some(e) = effort {
        args.extend(["--reasoning-effort".to_string(), e.to_string()]);
    }
    if !resumed {
        args.extend([
            "--rules".to_string(),
            crate::hooks::AUTO_TITLE_INSTRUCTION.to_string(),
        ]);
    }
    if let Some(p) = initial_prompt {
        args.push(p.to_string());
    }
}
