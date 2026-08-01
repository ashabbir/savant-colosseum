use serde_json::Value;

const FAILED_REVIEW_HEADING: &str = "## Colosseum review: failed";
const MAX_RECENT_COMMENTS: usize = 12;

pub(super) fn latest_failed_review(comments: &Value) -> Option<&str> {
    comments
        .as_array()?
        .iter()
        .rev()
        .filter_map(comment_text)
        .find(|text| text.contains(FAILED_REVIEW_HEADING))
}

pub(super) fn repair_context(comments: &Value) -> String {
    let Some(review) = latest_failed_review(comments) else {
        return String::new();
    };
    format!(
        concat!(
            "\n\n# Reviewer repair handoff\n",
            "This is a repair iteration. Treat the latest failed review below as a required checklist, ",
            "not background commentary. Before editing, also inspect the canonical ",
            "`code-reviews/**/review.md` artifact when present. Verify the current code for every prior ",
            "blocking finding, fix every unresolved item, and add regression coverage for each substantive ",
            "repair. Do not merely rerun validation. In the final summary, map each blocker to its fix or ",
            "explain why it is no longer applicable.\n\n{}",
        ),
        review.trim()
    )
}

pub(super) fn recent_activity(comments: &Value) -> Value {
    let Some(items) = comments.as_array() else {
        return Value::Array(vec![]);
    };
    let recent_start = items.len().saturating_sub(MAX_RECENT_COMMENTS);
    let failed_review = items.iter().enumerate().rev().find(|(_, item)| {
        comment_text(item).is_some_and(|text| text.contains(FAILED_REVIEW_HEADING))
    });
    if let Some((_, review)) = failed_review.filter(|(index, _)| *index < recent_start) {
        let mut selected = Vec::with_capacity(MAX_RECENT_COMMENTS);
        selected.push(review.clone());
        selected.extend(
            items[items.len().saturating_sub(MAX_RECENT_COMMENTS - 1)..]
                .iter()
                .cloned(),
        );
        return Value::Array(selected);
    }
    Value::Array(items[recent_start..].to_vec())
}

fn comment_text(comment: &Value) -> Option<&str> {
    comment.get("text").and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::{latest_failed_review, recent_activity, repair_context};
    use serde_json::{Value, json};

    #[test]
    fn repair_context_promotes_the_latest_failed_review_to_a_checklist() {
        let comments = json!([
            {"text":"## Colosseum review: failed\n\nOld blocker"},
            {"text":"work published"},
            {"text":"## Colosseum review: failed\n\n**Findings**\n- PID reuse"}
        ]);

        assert_eq!(
            latest_failed_review(&comments),
            Some("## Colosseum review: failed\n\n**Findings**\n- PID reuse")
        );
        let context = repair_context(&comments);
        assert!(context.contains("required checklist"));
        assert!(context.contains("PID reuse"));
        assert!(!context.contains("Old blocker"));
    }

    #[test]
    fn recent_activity_bounds_repeated_worker_chatter() {
        let comments = Value::Array(
            (0..20)
                .map(|index| json!({"text":format!("comment-{index}")}))
                .collect(),
        );
        let recent = recent_activity(&comments);
        let items = recent.as_array().unwrap();

        assert_eq!(items.len(), 12);
        assert_eq!(items.first().unwrap()["text"], "comment-8");
        assert_eq!(items.last().unwrap()["text"], "comment-19");
    }

    #[test]
    fn recent_activity_keeps_a_failed_review_when_chatter_would_displace_it() {
        let mut comments = vec![json!({
            "text":"## Colosseum review: failed\n\n**Findings**\n- Preserve me"
        })];
        comments.extend((0..20).map(|index| json!({"text":format!("chatter-{index}")})));

        let recent = recent_activity(&Value::Array(comments));
        let items = recent.as_array().unwrap();

        assert_eq!(items.len(), 12);
        assert!(
            items.first().unwrap()["text"]
                .as_str()
                .unwrap()
                .contains("Preserve me")
        );
        assert_eq!(items.last().unwrap()["text"], "chatter-19");
    }
}
