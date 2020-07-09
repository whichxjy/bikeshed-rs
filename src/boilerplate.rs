use kuchiki::traits::*;
use kuchiki::{NodeData, NodeDataRef, NodeRef};
use markup5ever::LocalName;
use std::fs;
use std::path::Path;

use crate::html::{self, Attr};
use crate::metadata::parse::Editor;
use crate::spec::Spec;

// Retrieve boilerplate file with doc (metadata).
fn retrieve_boilerplate(doc: &Spec, name: &str) -> String {
    if doc.md.boilerplate.get(name) {
        retrieve_boilerplate_with_info(name, doc.md.group.as_deref(), doc.md.raw_status.as_deref())
    } else {
        String::new()
    }
}

// Retrieve boilerplate file with group and status.
pub fn retrieve_boilerplate_with_info(
    name: &str,
    group: Option<&str>,
    status: Option<&str>,
) -> String {
    // File Priorities:
    // 1. [status file with group]
    // 2. [generic file with group]
    // 3. [status file without group]
    // 4. [generic file without group]

    let mut paths_to_try = Vec::new();

    let status_filename = if let Some(status) = status {
        Some(format!("{}-{}.include", name, status))
    } else {
        None
    };

    if let Some(ref status_filename) = status_filename {
        // status file with group
        if let Some(group) = group {
            paths_to_try.push(Path::new("boilerplate").join(group).join(status_filename));
        }
    }

    let generic_filename = format!("{}.include", name);

    if let Some(group) = group {
        // generic file with group
        paths_to_try.push(Path::new("boilerplate").join(group).join(&generic_filename));
    }

    if let Some(ref status_filename) = status_filename {
        // status file without group
        paths_to_try.push(Path::new("boilerplate").join(status_filename));
    }

    // generic file without group
    paths_to_try.push(Path::new("boilerplate").join(&generic_filename));

    for path in paths_to_try {
        if let Ok(data) = fs::read_to_string(path) {
            return data;
        }
    }

    die!("Can't find an appropriate include file for {}.", name);
}

pub fn load_containers(doc: &mut Spec) {
    if let Ok(container_els) = doc.dom().select("[data-fill-with]") {
        for container_el in container_els {
            doc.containers.insert(
                html::get_attr(container_el.as_node(), "data-fill-with").unwrap(),
                container_el.as_node().clone(),
            );
        }
    }
}

fn get_container(doc: &Spec, tag: &str) -> Option<NodeRef> {
    if !doc.md.boilerplate.get(tag) {
        return None;
    }

    doc.containers.get(tag).cloned()
}

fn get_container_or_head(doc: &Spec, tag: &str) -> Option<NodeRef> {
    if !doc.md.boilerplate.get(tag) {
        return None;
    }

    doc.containers
        .get(tag)
        .cloned()
        .or_else(|| Some(doc.head().clone()))
}

pub fn add_header_footer(doc: &mut Spec) {
    let header = retrieve_boilerplate(doc, "header");
    let footer = retrieve_boilerplate(doc, "footer");
    doc.html = [header, doc.html.clone(), footer].join("\n");
}

pub fn add_styles(doc: &mut Spec) {
    // TODO: Insert <style> nodes to body and move them to head later.
    let container = match get_container_or_head(doc, "bs-styles") {
        Some(container) => container,
        None => return,
    };

    for (key, val) in doc.extra_styles.iter() {
        if doc.md.boilerplate.get(*key) {
            container.append(html::new_style(format!("/* style-{} */\n\n{}", key, val)));
        }
    }
}

pub fn add_canonical_url(doc: &mut Spec) {
    if let Some(ref canonical_url) = doc.md.canonical_url {
        doc.head().append(html::new_element(
            "link",
            btreemap! {
                "rel" => "canonical",
                "href" => canonical_url,
            },
        ))
    }
}

// Convert an editor to a <dd> node.
fn editor_to_dd_node(editor: &Editor) -> NodeRef {
    let dd_el = html::new_element(
        "dd",
        btreemap! {
            "class" => "editor p-author h-card vcard",
        },
    );

    if let Some(ref w3c_id) = editor.w3c_id {
        if let NodeData::Element(dd_el_data) = dd_el.data() {
            let ref mut attributes = dd_el_data.attributes.borrow_mut();
            attributes.insert(LocalName::from("data-editor-id"), w3c_id.to_owned());
        }
    }

    if let Some(ref link) = editor.link {
        dd_el.append(html::new_a(
            btreemap! {
                "class" => "p-name fn u-url url",
                "href" => link,
            },
            &editor.name,
        ))
    } else if let Some(ref email) = editor.email {
        dd_el.append(html::new_a(
            btreemap! {
                "class" => "p-name fn u-email email".to_owned(),
                "href" => format!("mailto:{}", email),
            },
            &editor.name,
        ))
    } else {
        let span_el = html::new_element(
            "span",
            btreemap! {
                "class" => "p-name fn",
            },
        );
        span_el.append(html::new_text(&editor.name));
        dd_el.append(span_el);
    }

    if let Some(ref org) = editor.org {
        let el = if let Some(ref org_link) = editor.org_link {
            html::new_a(
                btreemap! {
                    "class" => "p-org org",
                    "href" => org_link,
                },
                org_link,
            )
        } else {
            let span_el = html::new_element(
                "span",
                btreemap! {
                    "class" => "p-org org",
                },
            );
            span_el.append(html::new_text(org.to_owned()));
            span_el
        };
        dd_el.append(html::new_text(" ("));
        dd_el.append(el);
        dd_el.append(html::new_text(")"));
    }

    if editor.link.is_some() {
        if let Some(ref email) = editor.email {
            dd_el.append(html::new_text(" "));
            dd_el.append(html::new_a(
                btreemap! {
                    "class" => "u-email email".to_owned(),
                    "href" => format!("mailto:{}", email),
                },
                email,
            ));
        }
    }

    dd_el
}

pub fn fill_spec_metadata_section(doc: &mut Spec) {
    let container = match get_container(doc, "spec-metadata") {
        Some(container) => container,
        None => return,
    };

    fn key_to_dt_node(key: &str) -> NodeRef {
        let dt_el = match key {
            "Editor" => html::new_element(
                "dt",
                btreemap! {
                    "class" => "editor"
                },
            ),
            _ => html::new_element("dt", None::<Attr>),
        };
        dt_el.append(html::new_text(format!("{}:", key)));
        dt_el
    }

    fn wrap_in_dd_node(el: NodeRef) -> NodeRef {
        let dd_el = html::new_element("dd", None::<Attr>);
        dd_el.append(el);
        dd_el
    }

    let macros = &doc.macros;

    // <dt> and <dd> nodes that would be appended to <dl> node
    let mut md_list = Vec::new();

    // insert version
    if let Some(version) = macros.get("version") {
        md_list.push(key_to_dt_node("This version"));
        md_list.push(wrap_in_dd_node(html::new_a(
            btreemap! {
                "class" => "u-url",
                "href" => version,
            },
            version,
        )));
    }

    // insert latest published version
    if let Some(ref tr) = doc.md.tr {
        md_list.push(key_to_dt_node("Latest published version"));
        md_list.push(wrap_in_dd_node(html::new_a(
            btreemap! {
                "href" => tr
            },
            tr,
        )));
    }

    // insert editors
    if !doc.md.editors.is_empty() {
        md_list.push(key_to_dt_node("Editor"));
        md_list.extend(doc.md.editors.iter().map(editor_to_dd_node));
    }

    // insert custom metadata
    for (key, vals) in &doc.md.custom_md {
        md_list.push(key_to_dt_node(key));
        md_list.extend(vals.iter().map(|val| wrap_in_dd_node(html::new_text(val))));
    }

    let dl_el = html::new_element("dl", None::<Attr>);

    for item in md_list {
        dl_el.append(item);
    }

    container.append(dl_el);
}

pub fn fill_copyright_section(doc: &mut Spec) {
    let container = match get_container(doc, "copyright") {
        Some(container) => container,
        None => return,
    };

    let mut copyright = retrieve_boilerplate(doc, "copyright");
    copyright = doc.fix_text(&copyright);
    let copyright_dom = kuchiki::parse_html().one(copyright);

    if let Ok(body) = copyright_dom.select_first("body") {
        for child in body.as_node().children() {
            container.append(child);
        }
    }
}

pub fn fill_abstract_section(doc: &mut Spec) {
    let container = match get_container(doc, "abstract") {
        Some(container) => container,
        None => return,
    };

    let mut abs = retrieve_boilerplate(doc, "abstract");
    abs = doc.fix_text(&abs);
    let abs_dom = kuchiki::parse_html().one(abs);

    if let Ok(body) = abs_dom.select_first("body") {
        for child in body.as_node().children() {
            container.append(child);
        }
    }
}

pub fn fill_toc_section(doc: &mut Spec) {
    let container = match get_container(doc, "table-of-contents") {
        Some(container) => container,
        None => return,
    };

    let h2_el = html::new_element(
        "h2",
        btreemap! {
            "class" => "no-num no-toc no-ref",
            "id" => "contents",
        },
    );
    h2_el.append(html::new_text("Table of Contents"));
    container.append(h2_el);

    // Each cell stores the reference to <ol> of a particular heading level.
    // Relation: <h[level]> => ol_cells[level - 2], where 2 <= level <= 6.
    let mut ol_cells: [Option<NodeRef>; 6] = Default::default();

    // Append a directory node (<ol> node) to table of contents, and then
    // store it to ol_cells[0].
    let dir_ol_el = html::new_element(
        "ol",
        btreemap! {
            "class" => "toc",
            "role"=> "directory",
        },
    );
    container.append(dir_ol_el.clone());
    ol_cells[0] = Some(dir_ol_el);

    let mut previous_level = 1;

    if let Ok(heading_els) = doc.dom().select("h2, h3, h4, h5, h6") {
        let heading_els = heading_els
            .map(|el| el.as_node().clone())
            .collect::<Vec<NodeRef>>();

        for heading_el in heading_els {
            let heading_tag = html::get_tag(&heading_el).unwrap();
            let curr_level = heading_tag.chars().last().unwrap().to_digit(10).unwrap() as usize;

            if curr_level > previous_level + 1 {
                die!(
                    "Heading level jumps more than one level, from h{} to h{}",
                    previous_level,
                    curr_level
                )
            }

            let curr_ol_el = if let Some(ref curr_ol_el) = ol_cells[curr_level - 2] {
                curr_ol_el
            } else {
                die!(
                    "Saw an <h{}> without seeing an <h{}> first. Please order your headings properly.",
                    curr_level,
                    curr_level - 1
                )
            };

            if html::has_class(&heading_el, "no-toc") {
                ol_cells[curr_level - 1] = None;
            } else {
                // Add a <li> node to current <ol> node.
                let a_el = {
                    let a_el = html::new_a(
                        btreemap! {
                            "href" => format!("#{}", html::get_attr(&heading_el, "id").unwrap())
                        },
                        "",
                    );

                    let span_el = html::new_element(
                        "span",
                        btreemap! {
                            "class"=>"secno"
                        },
                    );
                    span_el.append(html::new_text(
                        html::get_attr(&heading_el, "data-level").unwrap(),
                    ));
                    a_el.append(span_el);

                    a_el.append(html::new_text(" "));

                    if let Ok(content_el) = heading_el.select_first(".content") {
                        a_el.append(html::deep_clone(content_el.as_node()));
                    }

                    a_el
                };

                let li_el = html::new_element("li", None::<Attr>);
                li_el.append(a_el);

                let inner_ol_el = html::new_element(
                    "ol",
                    btreemap! {
                        "class" => "toc",
                    },
                );
                li_el.append(inner_ol_el.clone());

                curr_ol_el.append(li_el);

                ol_cells[curr_level - 1] = Some(inner_ol_el);
            }

            previous_level = curr_level;
        }
    }

    // Remove empty <ol> nodes.
    loop {
        if let Ok(ol_els) = container.select("ol:empty") {
            let ol_els = ol_els.collect::<Vec<NodeDataRef<_>>>();

            if ol_els.len() == 0 {
                break;
            }

            for ol_el in ol_els {
                ol_el.as_node().detach();
            }
        } else {
            break;
        }
    }
}
