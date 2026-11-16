use crate::context::Ctx;
use crate::vault::Item;

const SECOND_LEVEL: [&str; 7] = ["co", "com", "org", "net", "gov", "ac", "edu"];

pub fn host(uri: &str) -> String {
    let rest = uri.split_once("://").map_or(uri, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let h = authority.split(':').next().unwrap_or("").to_lowercase();
    if h.contains(' ') {
        String::new()
    } else {
        h.trim_start_matches("www.").to_string()
    }
}

pub fn label(host: &str) -> &str {
    let parts: Vec<&str> = host.split('.').collect();
    match parts.len() {
        0 | 1 => host,
        n if n >= 3 && SECOND_LEVEL.contains(&parts[n - 2]) => parts[n - 3],
        n => parts[n - 2],
    }
}

fn has_word(word: &str, text: &str) -> bool {
    let boundary =
        |c: Option<char>| !c.is_some_and(|c| c.is_alphanumeric() || c == '-' || c == '_');
    word.chars().count() > 2
        && text.match_indices(word).any(|(i, _)| {
            boundary(text[..i].chars().next_back())
                && boundary(text[i + word.len()..].chars().next())
        })
}

pub fn score(item: &Item, ctx: &Ctx) -> u8 {
    let page = host(&ctx.url);
    let text = format!("{} {}", ctx.title, ctx.app).to_lowercase();
    let mut best = 0;
    for uri in &item.uris {
        let h = host(uri);
        if h.is_empty() {
            continue;
        }
        if !page.is_empty()
            && (page == h || page.ends_with(&format!(".{h}")) || h.ends_with(&format!(".{page}")))
        {
            return 3;
        }
        if has_word(label(&h), &text) {
            best = 2;
        }
    }
    if best == 0 && has_word(&item.name.to_lowercase(), &text) {
        best = 1;
    }
    best
}

pub fn search(items: &[Item], query: &str, ctx: &Ctx) -> Vec<(u8, usize)> {
    let words: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(String::from)
        .collect();
    let mut found: Vec<(u8, usize)> = items
        .iter()
        .enumerate()
        .filter(|(_, i)| {
            let hay = format!("{} {} {}", i.name, i.username, i.uris.join(" ")).to_lowercase();
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .map(|(n, i)| (score(i, ctx), n))
        .collect();
    let name = |n: usize| items[n].name.to_lowercase();
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| name(a.1).cmp(&name(b.1))));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, uris: &[&str]) -> Item {
        Item {
            name: name.into(),
            username: "igor".into(),
            uris: uris.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    fn ctx(url: &str, title: &str, app: &str) -> Ctx {
        Ctx {
            url: url.into(),
            title: title.into(),
            app: app.into(),
            ..Default::default()
        }
    }

    #[test]
    fn host_and_label() {
        assert_eq!(host("https://www.GitHub.com/login"), "github.com");
        assert_eq!(host("accounts.google.com/x"), "accounts.google.com");
        assert_eq!(host("http://u:p@example.org:8080/a"), "example.org");
        assert_eq!(host("how to cook"), "");
        assert_eq!(label("bbc.co.uk"), "bbc");
        assert_eq!(label("mail.google.com"), "google");
    }

    #[test]
    fn url_match_beats_title() {
        let c = ctx("https://gist.github.com/x", "Gist", "Safari");
        assert_eq!(score(&item("GH", &["github.com"]), &c), 3);
        assert_eq!(score(&item("Other", &["gitlab.com"]), &c), 0);
    }

    #[test]
    fn title_match_uses_word_boundary() {
        let c = ctx("", "GitHub - Google Chrome", "chrome");
        assert_eq!(score(&item("GH", &["https://github.com"]), &c), 2);
        assert_eq!(score(&item("Git", &["git.io"]), &c), 0);
        assert_eq!(score(&item("Slack", &[]), &ctx("", "general", "slack")), 1);
        assert_eq!(
            score(&item("Почта", &[]), &ctx("", "Почта - Яндекс", "")),
            1
        );
    }

    #[test]
    fn search_filters_and_ranks() {
        let items = [
            item("Alpha", &["a.com"]),
            item("GitHub", &["github.com"]),
            item("Beta", &["b.com"]),
        ];
        let c = ctx("", "Pull requests · GitHub", "msedge");
        let names = |q| {
            search(&items, q, &c)
                .iter()
                .map(|&(_, n)| items[n].name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(""), ["GitHub", "Alpha", "Beta"]);
        assert_eq!(names("igor git"), ["GitHub"]);
    }
}
