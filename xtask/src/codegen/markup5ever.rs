use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

mod entities;

use crate::project_root;

use super::{ensure_file_contents, format_code, PREAMBLE};

static NAMESPACES: &[(&str, &str)] = &[
    ("", ""),
    ("*", "*"),
    ("html", "http://www.w3.org/1999/xhtml"),
    ("xml", "http://www.w3.org/XML/1998/namespace"),
    ("xmlns", "http://www.w3.org/2000/xmlns/"),
    ("xlink", "http://www.w3.org/1999/xlink"),
    ("svg", "http://www.w3.org/2000/svg"),
    ("mathml", "http://www.w3.org/1998/Math/MathML"),
];

pub fn generate(check: bool) -> anyhow::Result<()> {
    let mut contents = Vec::new();

    writeln!(&mut contents, "{PREAMBLE}")?;

    // Create a string cache for local names
    let local_names = project_root().join("markup5ever/local_names.txt");
    let mut local_names_atom = string_cache_codegen::AtomType::new("LocalName", "local_name!");
    for line in BufReader::new(File::open(local_names)?).lines() {
        let local_name = line?;
        local_names_atom.atom(&local_name);
        local_names_atom.atom(&local_name.to_ascii_lowercase());
    }
    local_names_atom
        .with_macro_doc("Takes a local name as a string and returns its key in the string cache.")
        .write_to(&mut contents)?;

    // Create a string cache for namespace prefixes
    string_cache_codegen::AtomType::new("Prefix", "namespace_prefix!")
        .with_macro_doc("Takes a namespace prefix string and returns its key in a string cache.")
        .atoms(NAMESPACES.iter().map(|&(prefix, _url)| prefix))
        .write_to(&mut contents)?;

    // Create a string cache for namespace urls
    string_cache_codegen::AtomType::new("Namespace", "namespace_url!")
        .with_macro_doc("Takes a namespace url string and returns its key in a string cache.")
        .atoms(NAMESPACES.iter().map(|&(_prefix, url)| url))
        .write_to(&mut contents)?;

    writeln!(
        contents,
        r#"
        /// Maps the input of [`namespace_prefix!`](macro.namespace_prefix.html) to
        /// the output of [`namespace_url!`](macro.namespace_url.html).
        ///
        #[macro_export] macro_rules! ns {{
        "#
    )?;
    for &(prefix, url) in NAMESPACES {
        writeln!(contents, "({prefix}) => {{ namespace_url!({url:?}) }};")?;
    }
    writeln!(contents, "}}")?;

    let generated_file = project_root().join("markup5ever/generated.rs");
    let contents = String::from_utf8(contents)?;
    let contents = format_code(&contents)?;
    ensure_file_contents(&generated_file, &contents, check)?;

    let named_entities = project_root().join("markup5ever/data/named_entities.rs");
    named_entities_to_phf(&named_entities, check)?;

    Ok(())
}

fn named_entities_to_phf(file: &Path, check: bool) -> anyhow::Result<()> {
    let mut entities: HashMap<&str, (u32, u32)> = entities::NAMED_ENTITIES
        .iter()
        .map(|(name, cp1, cp2)| {
            assert!(name.starts_with('&'));
            (&name[1..], (*cp1, *cp2))
        })
        .collect();

    // Add every missing prefix of those keys, mapping to NULL characters.
    for key in entities.keys().cloned().collect::<Vec<_>>() {
        for n in 1..key.len() {
            entities.entry(&key[..n]).or_insert((0, 0));
        }
    }
    entities.insert("", (0, 0));

    let mut phf_map = phf_codegen::Map::new();
    for (key, value) in entities {
        phf_map.entry(key, &format!("{value:?}"));
    }

    let mut contents = Vec::new();

    write!(&mut contents, "{PREAMBLE}")?;
    writeln!(
        &mut contents,
        r#"
/// A map of entity names to their codepoints. The second codepoint will
/// be 0 if the entity contains a single codepoint. Entities have their preceding '&' removed.
///
/// # Examples
///
/// ```
/// use markup5ever::data::NAMED_ENTITIES;
///
/// assert_eq!(NAMED_ENTITIES.get("gt;").unwrap(), &(62, 0));
/// ```
"#
    )?;
    writeln!(
        &mut contents,
        "pub static NAMED_ENTITIES: phf::Map<&'static str, (u32, u32)> = {};",
        phf_map.build(),
    )?;

    let contents = String::from_utf8(contents)?;
    let contents = format_code(&contents)?;
    ensure_file_contents(file, &contents, check)?;

    Ok(())
}
