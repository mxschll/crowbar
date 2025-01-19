use std::cell::{Ref, RefCell, RefMut};
use std::rc::{Rc, Weak};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageFormat {
    role: String,
    content: String,
}

#[derive(Debug, Clone)]
pub enum Role {
    User,
    System,
    Assistant,
}

impl Role {
    pub fn to_string(&self) -> String {
        match self {
            Role::User => "User".to_string(),
            Role::System => "System".to_string(),
            Role::Assistant => "Assistant".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    role: Role,
    content: String,
}

impl Message {
    pub fn new(role: Role, content: &str) -> Self {
        Self {
            role,
            content: content.to_string(),
        }
    }

    pub fn get_role(&self) -> Role {
        self.role.clone()
    }

    pub fn get_content(&self) -> String {
        self.content.clone()
    }

    pub fn append_message_content(&mut self, content: &str) {
        self.content.push_str(content);
    }
}

#[derive(Debug, Clone)]
pub struct ConversationNode {
    value: RefCell<Message>,
    parent: RefCell<Weak<ConversationNode>>,
    children: RefCell<Vec<Rc<ConversationNode>>>,
}

impl ConversationNode {
    pub fn new(value: Message) -> Rc<ConversationNode> {
        Rc::new(ConversationNode {
            value: RefCell::new(value),
            parent: RefCell::new(Weak::new()),
            children: RefCell::new(vec![]),
        })
    }

    pub fn borrow_message_mut(&self) -> RefMut<Message> {
        self.value.borrow_mut()
    }

    pub fn borrow_message(&self) -> Ref<Message> {
        self.value.borrow()
    }

    pub fn get_parent_ref(&self) -> RefCell<Weak<ConversationNode>> {
        self.parent.clone()
    }

    pub fn get_children_ref(&self) -> RefCell<Vec<Rc<ConversationNode>>> {
        self.children.clone()
    }

    // Returns root node of the tree
    pub fn get_root_node(&self) -> Rc<ConversationNode> {
        let mut current = Rc::new(self.clone());

        loop {
            let parent = current.parent.borrow().upgrade();
            match parent {
                Some(p) => current = p,
                None => break,
            }
        }

        current
    }

    // Prints the tree from the root node
    pub fn print_tree(&self) {
        fn print_node(node: &ConversationNode, depth: usize) {
            let indent_str = " ".repeat(depth * 2);
            println!("{}({}) {}", indent_str, depth, node.value.borrow().content);

            for child in node.children.borrow().iter() {
                print_node(child, depth + 1);
            }
        }

        // Get the root node and start printing from there
        let root = self.get_root_node();
        print_node(&root, 0);
    }

    /// Returns the depth of this node in the tree (0 for root)
    pub fn get_depth(&self) -> usize {
        let mut depth = 0;
        let mut current = Rc::new(self.clone());

        loop {
            let parent = current.parent.borrow().upgrade();
            match parent {
                Some(p) => {
                    depth += 1;
                    current = p;
                }
                None => break,
            }
        }

        depth
    }

    /// Returns a vector of nodes that have 2 or more children (branch nodes)
    pub fn get_branching_points(&self) -> Vec<Rc<ConversationNode>> {
        let mut branch_nodes = Vec::new();
        let mut nodes_to_visit = vec![Rc::new(self.clone())];

        while let Some(current_node) = nodes_to_visit.pop() {
            let children = current_node.children.borrow();

            // If node has 2 or more children, it's a branching point
            if children.len() >= 2 {
                branch_nodes.push(current_node.clone());
            }

            // Add all children to nodes_to_visit for traversal
            for child in children.iter() {
                nodes_to_visit.push(child.clone());
            }
        }

        branch_nodes
    }

    pub fn get_parent_nodes(self: &Rc<ConversationNode>) -> Vec<Rc<ConversationNode>> {
        let mut nodes = Vec::new();
        let mut current = Some(Rc::clone(self));

        while let Some(node) = current {
            nodes.push(Rc::clone(&node));
            current = node.parent.borrow().upgrade();
        }

        nodes.reverse();
        nodes
    }

    /// Returns a vector of MessageFormat objects representing the conversation path
    /// from the root to this node, in chronological order.
    pub fn get_conversation_context(&self) -> Vec<MessageFormat> {
        let mut messages = Vec::new();
        let mut current = Some(Rc::new(self.clone()));

        // First collect messages by walking up the tree to the root
        while let Some(node) = current {
            messages.push(node.value.borrow().clone());
            current = node.parent.borrow().upgrade().map(|p| p.clone());
        }

        // Reverse to get chronological order (root first)
        messages.reverse();

        // Convert to MessageFormat
        messages
            .into_iter()
            .map(|msg| MessageFormat {
                role: match msg.role {
                    Role::User => "user".to_string(),
                    Role::System => "system".to_string(),
                    Role::Assistant => "assistant".to_string(),
                },
                content: msg.content,
            })
            .collect()
    }

    pub fn add_child(self: &Rc<ConversationNode>, value: Message) -> Rc<ConversationNode> {
        let child = ConversationNode::new(value);
        *child.parent.borrow_mut() = Rc::downgrade(self);
        self.children.borrow_mut().push(Rc::clone(&child));
        child
    }

    pub fn set_value(&self, new_value: Message) {
        *self.value.borrow_mut() = new_value;
    }
}
