use std::collections::HashMap;

use alloy_primitives::{hex::ToHexExt, Selector};

use super::{modules::Module, tree::TreeNode};

pub(crate) fn render_tree<F>(
    root: &TreeNode,
    selectors: &HashMap<Selector, Module>,
    render_dynamic_result: F,
) -> String
where
    F: Fn(&Module) -> String,
{
    let mut tree = root.clone();
    let mut selectors_str: Vec<String> = Vec::new();

    fn render_node<F>(
        node: &mut TreeNode,
        indent: usize,
        selectors_str: &mut Vec<String>,
        selectors: &HashMap<Selector, Module>,
        render_dynamic_result: &F,
    ) where
        F: Fn(&Module) -> String,
    {
        if !node.children.is_empty() {
            let mut child_a = node.children.remove(0);
            let mut child_b = node.children.remove(0);

            fn find_mid_selector(node: &mut TreeNode) -> &Selector {
                if !node.selectors.is_empty() {
                    &node.selectors[0]
                } else {
                    find_mid_selector(&mut node.children[0])
                }
            }

            let mid_selector = find_mid_selector(&mut child_b);

            selectors_str.push(format!(
                "{}if lt(sig, {}) {{",
                repeat_string("    ", indent),
                mid_selector.encode_hex_with_prefix()
            ));
            render_node(
                &mut child_a,
                indent + 1,
                selectors_str,
                selectors,
                render_dynamic_result,
            );
            selectors_str.push(format!("{}}}", repeat_string("    ", indent)));

            render_node(
                &mut child_b,
                indent,
                selectors_str,
                selectors,
                render_dynamic_result,
            );
        } else {
            selectors_str.push(format!("{}switch sig", repeat_string("    ", indent)));
            for selector in &node.selectors {
                selectors_str.push(format!(
                    "{}{}",
                    repeat_string("    ", indent + 1),
                    render_dynamic_result(selectors.get(selector).unwrap())
                ));
            }
            selectors_str.push(format!("{}leave", repeat_string("    ", indent)));
        }
    }

    render_node(
        &mut tree,
        4,
        &mut selectors_str,
        selectors,
        &render_dynamic_result,
    );

    selectors_str.join("\n")
}

pub(crate) fn repeat_string(s: &str, count: usize) -> String {
    (0..count).map(|_| s).collect()
}
