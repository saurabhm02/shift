#![allow(dead_code)]


#[derive(Debug, PartialEq)]
pub enum Type{
    Post,
    Reel,
}

#[derive(Debug, PartialEq)]
pub struct Post {
    pub shortcode: String,
    pub url: String,
    pub post_type: Type,
    pub owner: String,
    pub caption: String,
    pub hashtags: Vec<String>,
}



pub fn read_url(url: &str) -> Option<(Type, String)> {
    let path = url.strip_prefix("https://www.instagram.com/")?;
    let mut parts = path.split('/'); 

    let post_type = match parts.next()? {
        "reel" => Type::Reel,
        "p" => Type::Post,
        _ => return None,
    }; 

   let shortcode = parts.next().filter(|code| !code.is_empty())?;
    Some((post_type, shortcode.to_string()))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_reel() {
        let url = "https://www.instagram.com/reel/DanFuOCIYD-/";
        assert_eq!(read_url(url), Some((Type::Reel, "DanFuOCIYD-".to_string())));
    }

    #[test]
    fn reads_a_post() {
        let url = "https://www.instagram.com/p/ABC123/";
        assert_eq!(read_url(url), Some((Type::Post, "ABC123".to_string())));
    }

    #[test]
    fn other_instagram_pages_are_not_posts() {
        assert_eq!(read_url("https://www.instagram.com/stories/someone/1/"), None);
    }

    #[test]
    fn other_sites_are_not_posts() {
        assert_eq!(read_url("https://example.com/reel/DanFuOCIYD-/"), None);
    }

    #[test]
    fn missing_shortcode_is_not_a_post() {
        assert_eq!(read_url("https://www.instagram.com/reel/"), None);
    }
}
