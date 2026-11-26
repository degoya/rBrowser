use rb_ipc::SitemapTreeNode;

use super::*;

fn urls(paths: &[&str]) -> Vec<String> {
    paths
        .iter()
        .map(|path| format!("https://a.test{path}"))
        .collect()
}

fn names(nodes: &[SitemapTreeNode]) -> Vec<&str> {
    nodes.iter().map(|node| node.name.as_str()).collect()
}

/// Every node's children split its range without a gap, in order.
fn assert_ranges(nodes: &[SitemapTreeNode], mut first: usize) -> usize {
    for node in nodes {
        assert_eq!(
            node.first, first,
            "{} starts where its sibling ended",
            node.name
        );
        if node.children.is_empty() {
            assert_eq!(node.count, 1, "{} is one page", node.name);
        } else {
            let end = assert_ranges(&node.children, node.first);
            assert_eq!(
                end,
                node.first + node.count,
                "{} counts its pages",
                node.name
            );
        }
        first += node.count;
    }
    first
}

#[test]
fn the_segments_make_folders_with_their_counts() {
    let tree = build(&urls(&[
        "/de/produkte/b",
        "/",
        "/about",
        "/de",
        "/de/produkte/a",
    ]));
    assert_eq!(names(&tree.nodes), ["https://a.test"]);
    let site = &tree.nodes[0];
    assert_eq!(site.count, 5);
    assert_eq!(names(&site.children), ["/", "about", "de"]);
    let de = &site.children[2];
    assert_eq!(de.count, 3);
    assert_eq!(names(&de.children), ["/de", "produkte"]);
    assert_eq!(names(&de.children[1].children), ["a", "b"]);
    assert_eq!(
        tree.pages,
        urls(&["/", "/about", "/de", "/de/produkte/a", "/de/produkte/b"])
    );
    assert_ranges(&tree.nodes, 0);
}

#[test]
fn a_subtree_is_one_range_of_the_pages() {
    let tree = build(&urls(&["/x/1", "/y", "/x/2", "/x/3/z", "/x"]));
    let x = &tree.nodes[0].children[0];
    assert_eq!(x.name, "x");
    let pages = &tree.pages[x.first..x.first + x.count];
    assert_eq!(pages, urls(&["/x", "/x/1", "/x/2", "/x/3/z"]).as_slice());
    assert_ranges(&tree.nodes, 0);
}

#[test]
fn a_query_makes_a_page_never_a_folder() {
    let tree = build(&urls(&[
        "/search?q=b",
        "/search?q=a",
        "/list/",
        "/list/1#top",
    ]));
    let site = &tree.nodes[0];
    assert_eq!(names(&site.children), ["list", "search?q=a", "search?q=b"]);
    assert_eq!(names(&site.children[0].children), ["/list/", "1"]);
    assert_eq!(tree.pages[1], "https://a.test/list/1#top");
    assert_ranges(&tree.nodes, 0);
}

#[test]
fn a_query_of_a_folder_page_stays_in_the_folder() {
    let tree = build(&urls(&["/search?q=a", "/search/advanced"]));
    let search = &tree.nodes[0].children[0];
    assert_eq!(names(&search.children), ["/search?q=a", "advanced"]);
}

#[test]
fn the_sites_keep_their_order_and_the_root_is_a_slash() {
    let list = vec![
        "https://b.test/".to_owned(),
        "https://a.test".to_owned(),
        "https://b.test/x".to_owned(),
        "https://a.test/?lang=de".to_owned(),
    ];
    let tree = build(&list);
    assert_eq!(names(&tree.nodes), ["https://b.test", "https://a.test"]);
    assert_eq!(names(&tree.nodes[0].children), ["/", "x"]);
    assert_eq!(names(&tree.nodes[1].children), ["/", "/?lang=de"]);
    assert_ranges(&tree.nodes, 0);
}

#[test]
fn a_deep_path_stops_making_folders() {
    let deep: String = (0..40).map(|n| format!("/s{n}")).collect();
    let tree = build(&urls(&[&deep, "/s0/other"]));
    let mut node = &tree.nodes[0];
    let mut levels = 0;
    while let Some(child) = node
        .children
        .iter()
        .find(|child| !child.children.is_empty())
    {
        node = child;
        levels += 1;
    }
    assert!(levels < MAX_DEPTH, "{levels} folder levels");
    assert_ranges(&tree.nodes, 0);
}

#[test]
fn ten_thousand_pages_make_one_tree() {
    let list: Vec<String> = (0..10_000)
        .map(|n| format!("https://a.test/s{}/p{}/{n}", n % 7, n % 31))
        .collect();
    let tree = build(&list);
    assert_eq!(tree.pages.len(), 10_000);
    assert_eq!(tree.nodes[0].count, 10_000);
    assert_eq!(tree.nodes[0].children.len(), 7);
    assert_ranges(&tree.nodes, 0);
}

#[test]
fn prune_cuts_the_levels_and_keeps_the_counts() {
    let mut tree = build(&urls(&["/a/b/c", "/a/b/d", "/e"]));
    prune(&mut tree.nodes, 2);
    let site = &tree.nodes[0];
    assert_eq!(names(&site.children), ["a", "e"]);
    assert!(site.children[0].children.is_empty());
    assert_eq!(site.children[0].count, 2);
}
