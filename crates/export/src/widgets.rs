//! Widgets of your own in an HTML export. The viewer is a static page that must never run what a folder shipped, so:
//! a declarative SVG widget goes in with its SVG made safe (scripts, foreign objects, images, media, event handlers and
//! anything pointing outside the document removed), and a code widget goes in as a value card (a declarative SVG that
//! shows the value, its unit and the widget's name). The page sanitises again when it draws; this is the part that
//! keeps a hostile file out of `export.json`.

use serde_json::{Value, json};

/// Tags removed with everything inside them.
const DROP: [&str; 12] = [
    "script",
    "foreignobject",
    "iframe",
    "object",
    "embed",
    "audio",
    "video",
    "image",
    "feimage",
    "style",
    "handler",
    "listener",
];

/// Animations, which can write an attribute after the attribute checks: dropped when they target a link, a handler or a
/// style.
const ANIMATE: [&str; 5] = ["set", "animate", "animatemotion", "animatetransform", "discard"];

/// `svg` without anything that runs or reaches outside the document.
pub fn sanitize_svg(svg: &str) -> String {
    let b = svg.as_bytes();
    let mut out = String::with_capacity(svg.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            let next = svg[i..].find('<').map_or(b.len(), |n| i + n);
            out.push_str(&svg[i..next]);
            i = next;
            continue;
        }
        // comments, CDATA and doctype declarations
        if svg[i..].starts_with("<!") {
            i = svg[i..].find('>').map_or(b.len(), |n| i + n + 1);
            continue;
        }
        let Some(end) = tag_end(svg, i) else { break };
        let tag = &svg[i + 1..end];
        let closing = tag.starts_with('/');
        let name_start = usize::from(closing);
        let name_len = tag[name_start..]
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .unwrap_or(tag.len() - name_start);
        let name = tag[name_start..name_start + name_len].to_ascii_lowercase();
        let bare = name.rsplit(':').next().unwrap_or(&name);
        if DROP.contains(&bare) || (ANIMATE.contains(&bare) && !closing && bad_animation(&tag[name_len..])) {
            i = end + 1;
            if !closing && !tag.ends_with('/') {
                // everything up to the matching close tag goes too
                let lower = svg[i..].to_ascii_lowercase();
                i = lower.find(&format!("</{name}")).map_or(b.len(), |n| {
                    let from = i + n;
                    svg[from..].find('>').map_or(b.len(), |m| from + m + 1)
                });
            }
            continue;
        }
        if closing || tag.starts_with('?') {
            out.push('<');
            out.push_str(tag);
            out.push('>');
        } else {
            out.push('<');
            out.push_str(&tag[..name_len]);
            out.push_str(&clean_attrs(&tag[name_len..]));
            out.push('>');
        }
        i = end + 1;
    }
    out
}

/// The index of the `>` that closes the tag opened at `from`, skipping quoted attribute values.
fn tag_end(s: &str, from: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (n, c) in s.bytes().enumerate().skip(from + 1) {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => quote = Some(c),
            (None, b'>') => return Some(n),
            _ => {}
        }
    }
    None
}

/// What a browser would read in an attribute value: character references decoded (`&#9;`, `&#x3a;`, `&colon;`), then
/// every space and control character dropped (a URL parser ignores a tab inside "java\tscript:"), lower case.
fn squeeze(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut rest = v;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let semi = rest.find(';').filter(|&n| n <= 12);
        let decoded = semi.and_then(|n| {
            let ent = &rest[1..n];
            let code = match ent.to_ascii_lowercase().as_str() {
                "tab" => Some(9),
                "newline" => Some(10),
                "colon" => Some(u32::from(b':')),
                "lpar" => Some(u32::from(b'(')),
                e if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16).ok(),
                e if e.starts_with('#') => e[1..].parse().ok(),
                _ => None,
            };
            code.and_then(char::from_u32).map(|c| (c, n))
        });
        match decoded {
            Some((c, n)) => {
                out.push(c);
                rest = &rest[n + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out.chars().filter(|c| !c.is_whitespace() && !c.is_control()).collect::<String>().to_lowercase()
}

/// Whether a squeezed value reaches outside the document: a script URL, a data URL, an @import, or a CSS url() to
/// anything but a fragment (`url(#grad)` stays; `url(//host/x)` is a beacon).
fn reaches_out(v: &str) -> bool {
    if ["javascript:", "vbscript:", "data:", "@import"].iter().any(|p| v.contains(p)) {
        return true;
    }
    v.match_indices("url(").any(|(n, _)| !v[n + 4..].trim_start_matches(['"', '\'']).starts_with('#'))
}

/// Whether an animation's attributes target a link, a handler or a style.
fn bad_animation(attrs: &str) -> bool {
    let lower = attrs.to_ascii_lowercase();
    let Some(at) = lower.find("attributename") else { return false };
    let v = lower[at + 13..].trim_start().strip_prefix('=').unwrap_or("").trim_start();
    let v = v.trim_start_matches(['"', '\'']);
    let t = squeeze(&v[..v.find(['"', '\'', ' ', '/', '>']).unwrap_or(v.len())]);
    t == "href" || t.ends_with(":href") || t.starts_with("on") || t == "style"
}

/// An attribute list without event handlers, outside links, script and data URLs, and CSS that reaches outside.
fn clean_attrs(attrs: &str) -> String {
    let mut out = String::new();
    let mut rest = attrs;
    let self_closing = attrs.trim_end().ends_with('/');
    while let Some(start) = rest.find(|c: char| !c.is_whitespace() && c != '/') {
        rest = &rest[start..];
        let name_len = rest.find(|c: char| c.is_whitespace() || c == '=' || c == '/').unwrap_or(rest.len());
        let name = &rest[..name_len];
        let mut after = rest[name_len..].trim_start();
        let mut value = None;
        if let Some(v) = after.strip_prefix('=') {
            let v = v.trim_start();
            let (val, left) = match v.chars().next() {
                Some(q @ ('"' | '\'')) => {
                    let inner = &v[1..];
                    let close = inner.find(q).unwrap_or(inner.len());
                    (&inner[..close], inner.get(close + 1..).unwrap_or(""))
                }
                _ => {
                    let close = v.find(|c: char| c.is_whitespace() || c == '/').unwrap_or(v.len());
                    (&v[..close], &v[close..])
                }
            };
            value = Some(val);
            after = left;
        }
        rest = after;
        let key = name.to_ascii_lowercase();
        let v = squeeze(value.unwrap_or(""));
        let outside = (key == "href" || key.ends_with(":href")) && !v.starts_with('#');
        if key.starts_with("on") || outside || reaches_out(&v) {
            continue;
        }
        out.push(' ');
        out.push_str(name);
        if let Some(val) = value {
            out.push_str(&format!("=\"{}\"", val.replace('"', "&quot;")));
        }
    }
    if self_closing {
        out.push('/');
    }
    out
}

/// A code widget as the value card the viewer shows in its place.
fn value_card(name: &str) -> String {
    let name: String = name.chars().filter(|c| !matches!(c, '<' | '>' | '&' | '"')).collect();
    format!(
        "<svg viewBox=\"0 0 120 60\"><text x=\"60\" y=\"30\" text-anchor=\"middle\" font-size=\"22\" fill=\"{{{{color}}}}\">{{{{value}}}}</text>\
         <text x=\"60\" y=\"46\" text-anchor=\"middle\" font-size=\"11\" fill=\"currentColor\" opacity=\".7\">{{{{unit}}}}</text>\
         <text x=\"60\" y=\"57\" text-anchor=\"middle\" font-size=\"9\" fill=\"currentColor\" opacity=\".5\">{name}</text></svg>"
    )
}

/// The widgets an export carries, from the ones the layout can use (`id`, `name`, `icon`, `svg`, `angle`, `code`,
/// `by`): SVG widgets sanitised, code widgets as value cards, nothing that runs.
pub fn export_widgets(widgets: &[Value]) -> Vec<Value> {
    widgets
        .iter()
        .filter_map(|w| {
            let id = w["id"].as_str()?;
            let name = w["name"].as_str().unwrap_or(id);
            let icon = w["icon"].as_str().map(sanitize_svg);
            let (svg, angle, card) = match (w["svg"].as_str(), w["code"].as_str()) {
                (Some(svg), None) => (sanitize_svg(svg), w["angle"].clone(), false),
                // code, with or without an SVG beside it, never travels
                _ => (value_card(name), Value::Null, true),
            };
            Some(json!({ "id": id, "name": name, "icon": icon, "svg": svg, "angle": angle, "code": null, "by": w["by"], "card": card }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_svg_keeps_its_drawing_and_loses_what_runs() {
        let hostile = r##"<svg viewBox="0 0 10 10" onload="x()"><script>alert(1)</script><g id="a" onclick='y()'><circle cx="5" cy="5" r="4" fill="{{color}}"/></g><foreignObject><div>hi</div></foreignObject><image href="http://e.com/a.png"/><use href="#a"/><use xlink:href="http://e.com/x.svg#a"/><rect width="1" height="1" style="fill:url(http://e.com/f)"/><a href="javascript:alert(1)"><text>{{value}}</text></a><!-- c --></svg>"##;
        let s = sanitize_svg(hostile);
        for bad in ["script", "alert", "onload", "onclick", "foreignObject", "<image", "e.com", "javascript", "<!--"] {
            assert!(!s.contains(bad), "{bad} survived in {s}");
        }
        for good in [
            "viewBox=\"0 0 10 10\"",
            "<circle cx=\"5\" cy=\"5\" r=\"4\" fill=\"{{color}}\"/>",
            "<use href=\"#a\"/>",
            "{{value}}",
            "id=\"a\"",
        ] {
            assert!(s.contains(good), "{good} lost from {s}");
        }
        // clean input comes out as it went in
        let clean = "<svg viewBox=\"0 0 1 1\"><path d=\"M0 0L1 1\" stroke=\"currentColor\"/></svg>";
        assert_eq!(sanitize_svg(clean), clean);
    }

    #[test]
    fn what_hides_from_a_plain_substring_check_goes_too() {
        // R6's security review: character references and spaces inside a scheme, animations that write a link or a
        // handler, beacons through protocol-relative CSS urls, data URLs and outside filter images
        let cases = [
            r##"<svg><a xlink:href="java&#9;script:alert(1)"><text>x</text></a></svg>"##,
            r##"<svg><a href="#"><set attributeName="href" to="java&#x0A;script:alert(1)"/><text>x</text></a></svg>"##,
            r##"<svg><a><animate attributeName="xlink:href" values="javascript&colon;alert(1)"></animate><text>x</text></a></svg>"##,
            r##"<svg><rect><set attributeName="onmouseover" to="alert(1)"/></rect></svg>"##,
            r##"<svg><rect style="fill: url(//e.com/beacon)"/></svg>"##,
            r##"<svg><rect fill="url(data:image/svg+xml;base64,AAAA)"/></svg>"##,
            r##"<svg><feImage href="//e.com/y.png"/></svg>"##,
        ];
        for c in cases {
            let s = sanitize_svg(c).to_ascii_lowercase();
            for bad in [
                "script",
                "alert",
                "e.com",
                "data:",
                "attributename=\"href",
                "attributename=\"xlink:href",
                "attributename=\"on",
                "feimage",
            ] {
                assert!(!s.contains(bad), "{bad} survived in {s} (from {c})");
            }
        }
        // what a widget legitimately uses stays
        let ok = r##"<svg viewBox="0 0 2 2"><rect fill="url(#g)" style="stroke: currentColor"/><animate attributeName="opacity" values="0;1" dur="1s"/><use href="#g"/></svg>"##;
        assert_eq!(sanitize_svg(ok), ok);
    }

    #[test]
    fn code_widgets_become_value_cards_and_nothing_carries_code() {
        let ws = vec![
            json!({ "id": "sw_bar", "name": "Level bar (code)", "code": "export default { build(el) { fetch('x') } }", "by": "j.kim" }),
            json!({ "id": "cw_gauge", "name": "Gauge", "svg": "<svg onload=\"x()\"><text>{{value}}</text></svg>", "angle": [-60, 60], "code": null }),
            json!({ "name": "no id" }),
        ];
        let out = export_widgets(&ws);
        assert_eq!(out.len(), 2, "a widget without an id is left out");
        assert!(out.iter().all(|w| w["code"].is_null()));
        assert_eq!(out[0]["card"], true);
        let card = out[0]["svg"].as_str().unwrap();
        assert!(card.contains("{{value}}") && card.contains("Level bar (code)") && !card.contains("fetch"));
        assert_eq!(out[1]["card"], false);
        assert!(!out[1]["svg"].as_str().unwrap().contains("onload") && out[1]["angle"] == json!([-60, 60]));
    }
}
