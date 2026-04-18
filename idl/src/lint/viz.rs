//! Graph visualization for account relationship graphs.
//!
//! Supports four output formats: ASCII (tree), Mermaid, DOT (Graphviz), and
//! JSON.  The ASCII renderer is the primary format shown in terminal output.

use {
    super::{constraints::FieldClass, graph::AccountGraph, types::GraphFormat},
    std::collections::{HashMap, HashSet},
};

/// Render the account graph in the requested format.
pub fn render(graph: &AccountGraph, format: &GraphFormat) -> String {
    match format {
        GraphFormat::Ascii => render_ascii(graph),
        GraphFormat::Mermaid => render_mermaid(graph),
        GraphFormat::Dot => render_dot(graph),
        GraphFormat::Json => render_json(graph),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Short label for node types that are self-constrained or otherwise notable.
fn node_label(class: &FieldClass) -> Option<&'static str> {
    match class {
        FieldClass::Signer => Some("signer"),
        FieldClass::Program => Some("program"),
        FieldClass::Sysvar => Some("sysvar"),
        FieldClass::Mint => Some("mint"),
        FieldClass::TokenAccount => Some("token"),
        FieldClass::SystemAccount => Some("system"),
        FieldClass::Unchecked => Some("unchecked"),
        _ => None,
    }
}

/// Format a node name with an optional type label, e.g. `authority [signer]`.
fn format_node(name: &str, class: &FieldClass) -> String {
    match node_label(class) {
        Some(label) => format!("{name} [{label}]"),
        None => name.to_string(),
    }
}

// ---------------------------------------------------------------------------
// ASCII renderer
// ---------------------------------------------------------------------------

fn render_ascii(graph: &AccountGraph) -> String {
    let mut out = String::new();

    out.push_str(&format!("  {}:\n\n", graph.struct_name));

    // Build lookup: node name -> &Node
    let node_map: HashMap<&str, &super::graph::Node> =
        graph.nodes.iter().map(|n| (n.name.as_str(), n)).collect();

    // Build outbound edges per node: name -> Vec<(target, label, is_missing)>
    let mut outbound: HashMap<&str, Vec<(&str, String, bool)>> = HashMap::new();
    for node in &graph.nodes {
        outbound.entry(node.name.as_str()).or_default();
    }
    for edge in &graph.edges {
        outbound.entry(edge.from.as_str()).or_default().push((
            edge.to.as_str(),
            edge.kind.label().to_string(),
            false,
        ));
    }
    for me in &graph.missing_edges {
        outbound.entry(me.from.as_str()).or_default().push((
            me.to.as_str(),
            me.expected_kind.label().to_string(),
            true,
        ));
    }

    // Find inbound node set (nodes that are targets of edges)
    let mut has_inbound: HashSet<&str> = HashSet::new();
    for edge in &graph.edges {
        has_inbound.insert(edge.to.as_str());
    }
    for me in &graph.missing_edges {
        has_inbound.insert(me.to.as_str());
    }

    // Root nodes: signers first, then nodes with no inbound edges, preserving
    // declaration order.
    let mut roots: Vec<&str> = Vec::new();
    // Signers first
    for node in &graph.nodes {
        if node.field_class == FieldClass::Signer {
            roots.push(node.name.as_str());
        }
    }
    // Then other nodes with no inbound edges (that aren't already roots)
    let root_set: HashSet<&str> = roots.iter().copied().collect();
    for node in &graph.nodes {
        if !root_set.contains(node.name.as_str()) && !has_inbound.contains(node.name.as_str()) {
            roots.push(node.name.as_str());
        }
    }

    // BFS tree rendering: walk from roots, visiting each node at most once
    let mut visited: HashSet<&str> = HashSet::new();

    for root in &roots {
        render_ascii_subtree(
            root,
            &node_map,
            &outbound,
            &mut visited,
            &mut out,
            4, // base indent
        );
    }

    // Any orphan nodes not yet visited (isolated nodes with inbound edges only
    // from visited nodes, or truly disconnected)
    for node in &graph.nodes {
        if !visited.contains(node.name.as_str()) {
            render_ascii_subtree(
                node.name.as_str(),
                &node_map,
                &outbound,
                &mut visited,
                &mut out,
                4,
            );
        }
    }

    // Edge summary
    let constrained = graph.constrained_edge_count();
    let total = graph.expected_edge_count();
    out.push_str(&format!("  {constrained}/{total} edges constrained\n"));

    out
}

/// Recursively render a node and its outbound edges as an ASCII tree.
fn render_ascii_subtree<'a>(
    name: &'a str,
    node_map: &HashMap<&'a str, &super::graph::Node>,
    outbound: &HashMap<&'a str, Vec<(&'a str, String, bool)>>,
    visited: &mut HashSet<&'a str>,
    out: &mut String,
    indent: usize,
) {
    // Cycle prevention: we should only visit each node once to avoid cycles.
    // But we need to handle the case where a node appears as the target of
    // multiple edges — we still print the edge, just don't recurse further.

    // We use a BFS-like approach: print the node, mark visited, then process
    // children depth-first.
    if visited.contains(name) {
        return;
    }
    visited.insert(name);

    let pad = " ".repeat(indent);

    // Print node header
    let class = node_map
        .get(name)
        .map(|n| &n.field_class)
        .unwrap_or(&FieldClass::Unchecked);
    let node_str = format_node(name, class);
    out.push_str(&format!("{pad}{node_str}\n"));

    // Print edges from this node
    let edges = match outbound.get(name) {
        Some(e) => e,
        None => return,
    };

    // We'll use a queue for deferred subtrees so we can show all edges from
    // this node together, then recurse.
    let mut subtree_queue: Vec<&str> = Vec::new();

    for (i, (target, label, is_missing)) in edges.iter().enumerate() {
        let is_last = i == edges.len() - 1;
        let connector = if is_last { "└" } else { "├" };

        if *is_missing {
            // Missing edge: show with red cross marker
            let child_pad = " ".repeat(indent + 2);
            out.push_str(&format!(
                "{child_pad}\u{2573}  missing: {label} = {target}\n"
            ));
        } else {
            let child_pad = " ".repeat(indent + 2);
            out.push_str(&format!("{child_pad}{connector}──{label}──→ {target}\n"));

            // Queue target for subtree rendering if not yet visited
            if !visited.contains(target) {
                subtree_queue.push(target);
            }
        }
    }

    // Render subtrees for targets (indented further under the parent)
    for target in subtree_queue {
        let child_indent = indent + format_node(name, class).len().min(20) + 2;
        render_ascii_subtree(target, node_map, outbound, visited, out, child_indent);
    }
}

// ---------------------------------------------------------------------------
// Mermaid renderer
// ---------------------------------------------------------------------------

fn render_mermaid(graph: &AccountGraph) -> String {
    let mut out = String::from("graph LR\n");

    // Declare nodes with labels
    for node in &graph.nodes {
        let icon = match &node.field_class {
            FieldClass::Signer => "\u{1f511} ",
            FieldClass::Program => "\u{2699}\u{fe0f} ",
            FieldClass::Sysvar => "\u{1f4cb} ",
            _ => "",
        };
        out.push_str(&format!("    {name}[\"{icon}{name}\"]\n", name = node.name));
    }

    // Edges
    for edge in &graph.edges {
        out.push_str(&format!(
            "    {} -->|{}| {}\n",
            edge.from,
            edge.kind.label(),
            edge.to,
        ));
    }

    // Missing edges (dashed)
    for me in &graph.missing_edges {
        out.push_str(&format!(
            "    {} -.->|MISSING: {}| {}\n",
            me.from,
            me.expected_kind.label(),
            me.to,
        ));
    }

    out
}

// ---------------------------------------------------------------------------
// DOT renderer
// ---------------------------------------------------------------------------

fn render_dot(graph: &AccountGraph) -> String {
    let mut out = format!("digraph {} {{\n", graph.struct_name);
    out.push_str("    rankdir=LR;\n");

    // Node declarations
    for node in &graph.nodes {
        let shape = match &node.field_class {
            FieldClass::Signer => "diamond",
            FieldClass::Program | FieldClass::Sysvar => "ellipse",
            _ => "box",
        };
        out.push_str(&format!("    {} [shape={}];\n", node.name, shape));
    }

    // Edges
    for edge in &graph.edges {
        out.push_str(&format!(
            "    {} -> {} [label=\"{}\"];\n",
            edge.from,
            edge.to,
            edge.kind.label(),
        ));
    }

    // Missing edges
    for me in &graph.missing_edges {
        out.push_str(&format!(
            "    {} -> {} [label=\"MISSING: {}\" style=dashed color=red];\n",
            me.from,
            me.to,
            me.expected_kind.label(),
        ));
    }

    out.push_str("}\n");
    out
}

// ---------------------------------------------------------------------------
// JSON renderer
// ---------------------------------------------------------------------------

fn render_json(graph: &AccountGraph) -> String {
    let nodes: Vec<&str> = graph.nodes.iter().map(|n| n.name.as_str()).collect();

    let edges: Vec<serde_json::Value> = graph
        .edges
        .iter()
        .map(|e| {
            serde_json::json!({
                "from": e.from,
                "to": e.to,
                "kind": e.kind.label(),
            })
        })
        .collect();

    let missing: Vec<serde_json::Value> = graph
        .missing_edges
        .iter()
        .map(|me| {
            serde_json::json!({
                "from": me.from,
                "to": me.to,
                "expected_kind": me.expected_kind.label(),
            })
        })
        .collect();

    let doc = serde_json::json!({
        "struct": graph.struct_name,
        "nodes": nodes,
        "edges": edges,
        "missing_edges": missing,
    });

    serde_json::to_string_pretty(&doc).expect("JSON serialization should not fail")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::lint::{
            constraints::FieldConstraints,
            graph::{AccountGraph, Edge, EdgeKind, MissingEdge, Node},
        },
    };

    fn sample_graph() -> AccountGraph {
        AccountGraph {
            struct_name: "Approve".to_string(),
            nodes: vec![
                Node {
                    name: "authority".to_string(),
                    field_class: FieldClass::Signer,
                    inner_type_name: None,
                    constraints: FieldConstraints::default(),
                    writable: false,
                },
                Node {
                    name: "vault".to_string(),
                    field_class: FieldClass::Account {
                        inner_type: "Vault".to_string(),
                    },
                    inner_type_name: Some("Vault".to_string()),
                    constraints: FieldConstraints::default(),
                    writable: true,
                },
                Node {
                    name: "token_account".to_string(),
                    field_class: FieldClass::TokenAccount,
                    inner_type_name: None,
                    constraints: FieldConstraints::default(),
                    writable: true,
                },
                Node {
                    name: "mint".to_string(),
                    field_class: FieldClass::Mint,
                    inner_type_name: None,
                    constraints: FieldConstraints::default(),
                    writable: false,
                },
            ],
            edges: vec![
                Edge {
                    from: "authority".to_string(),
                    to: "vault".to_string(),
                    kind: EdgeKind::HasOne,
                },
                Edge {
                    from: "authority".to_string(),
                    to: "token_account".to_string(),
                    kind: EdgeKind::TokenAuthority,
                },
                Edge {
                    from: "token_account".to_string(),
                    to: "mint".to_string(),
                    kind: EdgeKind::TokenMint,
                },
            ],
            missing_edges: vec![MissingEdge {
                from: "vault".to_string(),
                to: "authority".to_string(),
                expected_kind: EdgeKind::HasOne,
                address_field: "authority".to_string(),
            }],
        }
    }

    #[test]
    fn ascii_contains_struct_name() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Ascii);
        assert!(out.contains("Approve:"), "should contain struct name");
    }

    #[test]
    fn ascii_contains_signer_label() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Ascii);
        assert!(out.contains("[signer]"), "should label signer nodes: {out}");
    }

    #[test]
    fn ascii_contains_edge_arrow() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Ascii);
        assert!(
            out.contains("has_one──→ vault"),
            "should show has_one edge: {out}"
        );
    }

    #[test]
    fn ascii_contains_missing_marker() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Ascii);
        assert!(
            out.contains("\u{2573}") && out.contains("missing:"),
            "should show missing edge marker: {out}"
        );
    }

    #[test]
    fn ascii_contains_edge_count() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Ascii);
        assert!(
            out.contains("3/4 edges constrained"),
            "should show 3/4 edges: {out}"
        );
    }

    #[test]
    fn mermaid_has_graph_header() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Mermaid);
        assert!(out.starts_with("graph LR"), "should start with graph LR");
    }

    #[test]
    fn mermaid_has_edges() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Mermaid);
        assert!(
            out.contains("authority -->|has_one| vault"),
            "should contain edge: {out}"
        );
    }

    #[test]
    fn mermaid_has_missing_edges() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Mermaid);
        assert!(
            out.contains("-.->|MISSING:"),
            "should contain dashed missing edge: {out}"
        );
    }

    #[test]
    fn dot_has_digraph_header() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Dot);
        assert!(
            out.starts_with("digraph Approve {"),
            "should start with digraph: {out}"
        );
    }

    #[test]
    fn dot_has_edges() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Dot);
        assert!(
            out.contains("authority -> vault [label=\"has_one\"]"),
            "should contain edge: {out}"
        );
    }

    #[test]
    fn dot_has_missing_edges() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Dot);
        assert!(
            out.contains("MISSING:") && out.contains("style=dashed"),
            "should contain dashed missing edge: {out}"
        );
    }

    #[test]
    fn dot_has_signer_shape() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Dot);
        assert!(
            out.contains("authority [shape=diamond]"),
            "signer should have diamond shape: {out}"
        );
    }

    #[test]
    fn json_parses() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Json);
        let v: serde_json::Value = serde_json::from_str(&out).expect("should be valid JSON");
        assert_eq!(v["struct"], "Approve");
        assert_eq!(v["nodes"].as_array().unwrap().len(), 4);
        assert_eq!(v["edges"].as_array().unwrap().len(), 3);
        assert_eq!(v["missing_edges"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn json_edge_fields() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Json);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let first_edge = &v["edges"][0];
        assert_eq!(first_edge["from"], "authority");
        assert_eq!(first_edge["to"], "vault");
        assert_eq!(first_edge["kind"], "has_one");
    }

    #[test]
    fn json_missing_edge_fields() {
        let g = sample_graph();
        let out = render(&g, &GraphFormat::Json);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let me = &v["missing_edges"][0];
        assert_eq!(me["from"], "vault");
        assert_eq!(me["to"], "authority");
        assert_eq!(me["expected_kind"], "has_one");
    }

    #[test]
    fn empty_graph() {
        let g = AccountGraph {
            struct_name: "Empty".to_string(),
            nodes: vec![],
            edges: vec![],
            missing_edges: vec![],
        };
        let ascii = render(&g, &GraphFormat::Ascii);
        assert!(ascii.contains("0/0 edges constrained"));

        let json = render(&g, &GraphFormat::Json);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["nodes"].as_array().unwrap().len(), 0);
    }
}
