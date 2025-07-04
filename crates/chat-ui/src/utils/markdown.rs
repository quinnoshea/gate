use yew::prelude::*;

pub fn render_markdown(content: &str) -> Html {
    // For now, let's do basic markdown rendering without the pulldown-cmark dependency
    // to avoid version conflicts. We can enhance this later.
    let mut html_output = String::new();
    let mut in_code_block = false;
    let mut code_content = String::new();

    for line in content.lines() {
        if line.starts_with("```") {
            if in_code_block {
                // End code block
                html_output.push_str("<pre><code>");
                html_output.push_str(&html_escape(&code_content));
                html_output.push_str("</code></pre>");
                code_content.clear();
                in_code_block = false;
            } else {
                // Start code block
                in_code_block = true;
            }
        } else if in_code_block {
            if !code_content.is_empty() {
                code_content.push('\n');
            }
            code_content.push_str(line);
        } else {
            // Simple replacements for basic markdown
            let mut processed_line = line.to_string();

            // Headers
            if let Some(rest) = processed_line.strip_prefix("### ") {
                html_output.push_str(&format!("<h3>{}</h3>", html_escape(rest)));
            } else if let Some(rest) = processed_line.strip_prefix("## ") {
                html_output.push_str(&format!("<h2>{}</h2>", html_escape(rest)));
            } else if let Some(rest) = processed_line.strip_prefix("# ") {
                html_output.push_str(&format!("<h1>{}</h1>", html_escape(rest)));
            } else if processed_line.trim().is_empty() {
                // Empty line
                if !html_output.ends_with("</p>") && !html_output.ends_with("</pre>") {
                    html_output.push_str("<br>");
                }
            } else {
                // Regular paragraph
                html_output.push_str("<p>");

                // Bold text
                processed_line = replace_pattern(&processed_line, "**", "<strong>", "</strong>");

                // Italic text
                processed_line = replace_pattern(&processed_line, "*", "<em>", "</em>");

                // Inline code
                processed_line = replace_inline_code(&processed_line);

                html_output.push_str(&processed_line);
                html_output.push_str("</p>");
            }
        }
    }

    // Handle unclosed code block
    if in_code_block {
        html_output.push_str("<pre><code>");
        html_output.push_str(&html_escape(&code_content));
        html_output.push_str("</code></pre>");
    }

    Html::from_html_unchecked(AttrValue::from(html_output))
}

fn replace_pattern(text: &str, delimiter: &str, open_tag: &str, close_tag: &str) -> String {
    let parts: Vec<&str> = text.split(delimiter).collect();
    if parts.len() < 3 {
        return text.to_string();
    }

    let mut result = String::new();
    let mut in_delimiter = false;

    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            result.push_str(part);
        } else if in_delimiter {
            result.push_str(close_tag);
            result.push_str(part);
            in_delimiter = false;
        } else {
            result.push_str(open_tag);
            result.push_str(part);
            in_delimiter = true;
        }
    }

    result
}

fn replace_inline_code(text: &str) -> String {
    let parts: Vec<&str> = text.split('`').collect();
    if parts.len() < 3 {
        return text.to_string();
    }

    let mut result = String::new();
    let mut in_code = false;

    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            result.push_str(part);
        } else if in_code {
            result.push_str("</code>");
            result.push_str(part);
            in_code = false;
        } else {
            result.push_str("<code>");
            result.push_str(&html_escape(part));
            in_code = true;
        }
    }

    result
}

fn html_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&#39;".to_string(),
            _ => c.to_string(),
        })
        .collect()
}
