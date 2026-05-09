use quick_xml::escape::unescape;
use quick_xml::events::Event;
use quick_xml::Reader;

pub fn build_packet(tags: &[String]) -> String {
    let mut items = String::with_capacity(tags.len() * 32);
    for tag in tags {
        items.push_str("      <rdf:li>");
        push_escaped(&mut items, tag);
        items.push_str("</rdf:li>\n");
    }

    // U+FEFF (BOM) is mandated by the XMP packet wrapper specification.
    format!(
        "<?xpacket begin='\u{feff}' id='W5M0MpCehiHzreSzNTczkc9d'?>\n\
         <x:xmpmeta xmlns:x='adobe:ns:meta/'>\n  \
         <rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'>\n    \
         <rdf:Description xmlns:dc='http://purl.org/dc/elements/1.1/'>\n      \
         <dc:subject>\n        \
         <rdf:Bag>\n\
         {items}        </rdf:Bag>\n      \
         </dc:subject>\n    \
         </rdf:Description>\n  \
         </rdf:RDF>\n\
         </x:xmpmeta>\n\
         <?xpacket end='w'?>"
    )
}

pub fn parse_subjects(xmp: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xmp);
    reader.trim_text(true);

    let mut tags = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref().ends_with(b"li") => {
                // `read_text` returns the raw payload (with XML entities still escaped).
                // Decode them so callers see the original tag string.
                if let Ok(raw) = reader.read_text(e.name()) {
                    let decoded = unescape(&raw).unwrap_or_else(|_| raw.clone());
                    let trimmed = decoded.trim();
                    if !trimmed.is_empty() {
                        tags.push(trimmed.to_string());
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    tags
}

fn push_escaped(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_then_parse_round_trips_simple_tags() {
        let tags = vec!["cat".to_string(), "outdoor".to_string()];
        let packet = build_packet(&tags);
        let parsed = parse_subjects(&packet);
        assert_eq!(parsed, tags);
    }

    #[test]
    fn special_characters_are_escaped_and_decoded() {
        let tags = vec!["a&b".to_string(), "<x>".to_string(), "\"q\"".to_string()];
        let packet = build_packet(&tags);
        // Raw entities must appear in the packet
        assert!(packet.contains("a&amp;b"));
        assert!(packet.contains("&lt;x&gt;"));
        // Round-trip must yield the original strings (quick-xml decodes entities).
        let parsed = parse_subjects(&packet);
        assert_eq!(parsed, tags);
    }

    #[test]
    fn empty_tag_list_round_trips_to_empty() {
        let packet = build_packet(&[]);
        assert!(parse_subjects(&packet).is_empty());
    }

    #[test]
    fn unicode_tags_round_trip() {
        let tags = vec!["猫".to_string(), "犬と猫".to_string()];
        let packet = build_packet(&tags);
        let parsed = parse_subjects(&packet);
        assert_eq!(parsed, tags);
    }
}
