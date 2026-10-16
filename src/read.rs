use scraper::{ElementRef, Html, Selector};

use crate::post::{Post, read_url};

fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("css selector is written by us and is valid")
}

fn text(element: ElementRef) -> String {
    element.text().collect::<String>().trim().to_string()
}

pub fn read_posts(html: &str) -> Vec<Post> {
    let page = Html::parse_document(html);
    let link = selector("a[href]");

    page.select(&selector("table"))
        .filter(|table| table.select(&link).next().is_some())
        .filter_map(read_post)
        .collect()
}

fn read_post(table: ElementRef) -> Option<Post> {
    let url = table
        .select(&selector("a[href]"))
        .next()?
        .value()
        .attr("href")?;
    let (post_type, shortcode) = read_url(url)?;

    let mut caption = None;
    let mut owner = String::new();
    for row in table.select(&selector("tr")) {
        let cells: Vec<ElementRef> = row.children().filter_map(ElementRef::wrap).collect();
        if let [label, value] = cells.as_slice() {
            match text(*label).as_str() {
                "Caption" if caption.is_none() => caption = Some(text(*value)),
                "Username" => owner = text(*value),
                _ => {}
            }
        }
    }

    let hashtags = table
        .select(&selector("h2"))
        .find(|heading| text(*heading) == "Hashtags")
        .and_then(|heading| ElementRef::wrap(heading.parent()?))
        .map(|section| {
            section
                .select(&selector("div._a6-p"))
                .filter(|div| div.children().all(|child| child.value().is_text()))
                .map(text)
                .filter(|name| !name.is_empty())
                .collect()
        })
        .unwrap_or_default();

    Some(Post {
        shortcode,
        url: url.to_string(),
        post_type,
        owner,
        caption: caption.unwrap_or_default(),
        hashtags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::post::Type;

    const ONE_REEL: &str = r#"<table><tr><td colspan="2" class="_a6_q">URL<div><a href="https://www.instagram.com/reel/DanFuOCIYD-/">x</a></div></td></tr>
<tr><td class="_a6_q">Caption</td><td class="_2piu _a6_r">Hello world #a</td></tr>
<tr><td class="_a6_q">Caption</td><td class="_2piu _a6_r">Hello world #a</td></tr>
<tr><td colspan="2" class="_a6_q"><div><div class="pam"><h2>Hashtags</h2><div class="_a6-p"><div><div class="_2ph_ _a6_q">Name</div><div><div><div class="pam"><div class="_a6-p">sarabjeetsachar</div></div></div><div><div class="pam"><div class="_a6-p">careeradvice</div></div></div></div></div></div></div></div></td></tr>
<tr><td colspan="2" class="_a6_q"><div><div class="pam"><h2>Owner</h2><div class="_a6-p"><table><tr><td colspan="2"><table><tr><td class="_a6_q">URL</td><td class="_2piu _a6_r">https://youtu.be/x</td></tr><tr><td class="_a6_q">Name</td><td class="_2piu _a6_r">Sarabjeet Sachar</td></tr><tr><td class="_a6_q">Username</td><td class="_2piu _a6_r">sarabjeetsachar</td></tr></table></td></tr></table></div></div></div></td></tr></table>"#;

    const NO_CAPTION: &str = r#"<table><tr><td colspan="2" class="_a6_q">URL<div><a href="https://www.instagram.com/p/ABC123/">x</a></div></td></tr></table>"#;

    #[test]
    fn reads_every_field_of_one_post() {
        let posts = read_posts(ONE_REEL);
        assert_eq!(posts.len(), 1);
        let post = &posts[0];
        assert_eq!(post.shortcode, "DanFuOCIYD-");
        assert_eq!(post.post_type, Type::Reel);
        assert_eq!(post.owner, "sarabjeetsachar");
        assert_eq!(post.caption, "Hello world #a");
        assert_eq!(post.hashtags, vec!["sarabjeetsachar", "careeradvice"]);
    }

    #[test]
    fn post_without_caption_has_empty_caption() {
        let posts = read_posts(NO_CAPTION);
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].caption, "");
        assert!(posts[0].hashtags.is_empty());
    }

    #[test]
    fn two_posts_in_one_page() {
        let both = format!("{ONE_REEL}{NO_CAPTION}");
        assert_eq!(read_posts(&both).len(), 2);
    }
}
