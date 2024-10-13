use alloy_primitives::Selector;

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub selectors: Vec<Selector>,
    pub children: Vec<TreeNode>,
}

pub fn build_binary_tree(selectors: Vec<Selector>) -> TreeNode {
    const MAX_SELECTORS_PER_SWITCH_STATEMENT: usize = 9;

    let mut selectors = selectors;
    selectors.sort_by(|a, b| a.cmp(&b));

    fn binary_split(node: &mut TreeNode) {
        if node.selectors.len() > MAX_SELECTORS_PER_SWITCH_STATEMENT {
            let mid_idx = (node.selectors.len() + 1) / 2;

            let mut child_a = TreeNode {
                selectors: node.selectors.drain(..mid_idx).collect(),
                children: Vec::new(),
            };

            let mut child_b = TreeNode {
                selectors: node.selectors.drain(..).collect(),
                children: Vec::new(),
            };

            binary_split(&mut child_a);
            binary_split(&mut child_b);

            node.children.push(child_a);
            node.children.push(child_b);
        }
    }

    let mut root = TreeNode {
        selectors,
        children: Vec::new(),
    };

    binary_split(&mut root);

    root
}
