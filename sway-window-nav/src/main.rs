use std::env;
use std::cmp;
use std::fmt;
use swayipc::Connection;
use swayipc::{NodeLayout, NodeType};
use anyhow::{anyhow, bail, Result};

#[cfg(debug_assertions)]
macro_rules! dbg_println {
    ($( $args:expr ),*) => { println!( $( $args ),* ); }
}
#[cfg(not(debug_assertions))]
macro_rules! dbg_println {
    ($( $args:expr ),*) => {}
}

#[cfg(debug_assertions)]
macro_rules! dbg_dbg {
    ($( $args:expr ),*) => { dbg!( $( $args ),* ); }
}
#[cfg(not(debug_assertions))]
macro_rules! dbg_dbg {
    ($( $args:expr ),*) => {}
}
// Sheesh!

macro_rules! is_node_leaf {
    ($n:expr) => { $n.nodes.is_empty() }
}


// y: 0 = top, increases down
// x: 0 = left, increases right
#[derive(Debug, Eq, PartialEq)]
struct Coord {
    x: i32,
    y: i32,
}
impl fmt::Display for Coord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:>4}, {:>4})", self.x, self.y)
    }
}
impl From<&swayipc::Rect> for Coord {
    fn from(r: &swayipc::Rect) -> Self {
        Self {
            x: r.x,
            y: r.y,
        }
    }
}

impl Coord {
    #[allow(dead_code)]
    fn asc_below_or_right_of(&self, b: &Coord) -> cmp::Ordering {
        if self == b {
            cmp::Ordering::Equal
        } else if self.y > b.y || self.x < b.x {
            cmp::Ordering::Less
        } else {
            cmp::Ordering::Greater
        }
    }

    fn asc_above_or_right_of(&self, b: &Coord) -> cmp::Ordering {
        if self == b {
            cmp::Ordering::Equal
        } else if self.y < b.y || self.x < b.x {
            cmp::Ordering::Less
        } else {
            cmp::Ordering::Greater
        }
    }
}

enum CommandArgument {
    Next,
    Prev,
}
enum Command {
    Focus,
    Move,
}

// Used to squeeze out only the fields we care about from swayipc::Node, and to help testing.
#[derive(Debug)]
struct Node {
    id: i64,
    name: Option<String>,
    focused: bool,
    coords: Coord,
    deco_coords: Coord,
    nodes: Vec<Node>,
}
impl From<swayipc::Node> for Node {
    fn from(n: swayipc::Node) -> Self {
        // Check if these assertions hold?
        debug_assert!(!is_node_leaf!(n) || n.name.is_some());
        debug_assert!(!is_node_leaf!(n) || n.visible.is_some());
        debug_assert!(!is_node_leaf!(n) || n.layout == NodeLayout::None);
        debug_assert!(n.layout != NodeLayout::Output);
        debug_assert!(n.layout != NodeLayout::Dockarea);
        let c = Coord {
            x: n.rect.x,
            // Parent nodes include the deco_rect coordinates in the rect (bug in sway?).
            y: if is_node_leaf!(n) {
                n.rect.y
            } else {
                n.rect.y - n.deco_rect.y
            }
        };

        Self {
            id: n.id,
            name: n.name,
            focused: n.focused,
            coords: c,
            deco_coords: Coord::from(&n.deco_rect),
            nodes: n.nodes.into_iter().chain(n.floating_nodes.into_iter()).map(Node::from).collect(),
        }
    }
}
impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if is_node_leaf!(self) {
            write!(f, "{}: {}, {}", self.id, self.coords, self.deco_coords)
        } else {
            let mut pstr = String::new();
            for p in &self.nodes {
                pstr.push_str(&format!("{}, ", p))
            }
            write!(f, "{}: {}, {} [{}]", self.id, self.coords, self.deco_coords, pstr)
        }
    }
}
impl Node {
    #[allow(dead_code)]
    fn asc_below_or_right_of(&self, b: &Node) -> cmp::Ordering {
        self.coords.asc_below_or_right_of(&b.coords).reverse().then_with(|| {
            // Depth is the same, we don't care about y.
            // Tabbed parent inside a tab has deco_rect y set (incorrectly, bug in sway?).
            let a = self.deco_coords.x;
            let b = b.deco_coords.x;
            b.cmp(&a)
        })
    }

    fn asc_above_or_right_of(&self, b: &Node) -> cmp::Ordering {
        self.coords.asc_above_or_right_of(&b.coords).reverse().then_with(|| {
            // Depth is the same, we don't care about y.
            // Tabbed parent inside a tab has deco_rect y set (incorrectly, bug in sway?).
            let a = self.deco_coords.x;
            let b = b.deco_coords.x;
            b.cmp(&a)
        })
    }
}

fn main()  -> Result<()> {
    //
    // Lazy arg parsing.
    //
    let args: Vec<String> = env::args().take(3).collect();

    if args.len() < 3 {
        let exe_path = env::current_exe()?;
        let exe = exe_path.file_name().ok_or_else(|| anyhow!("Could not get executable file name."))?.to_string_lossy();
        bail!("{} focus|move next|prev", exe);
    }

    let cmd = match args[1].as_str() {
        "focus" => Command::Focus,
        "move" => Command::Move,
        _ => bail!("First argument accepts only focus or move.")
    };
    let cmd_arg = match args[2].as_str() {
        "next" => CommandArgument::Next,
        "prev" => CommandArgument::Prev,
        _ => bail!("Second argument accepts only next or prev.")
    };

    //
    // Extract the currently focused workspace out of get_tree.
    //
    let mut conn = Connection::new()?;
    let mut node = conn.get_tree()?;
    assert!(node.node_type == NodeType::Root);

    while node.node_type != NodeType::Workspace {
        let fid = node.focus.into_iter().next().ok_or_else(|| anyhow!("Could not find a focused output or workspace."))?;

        // I suppose workspaces can't be in floating_nodes...
        node = node.nodes.into_iter().find(|n| n.id == fid)
            .ok_or_else(|| anyhow!("Could not find the focused workspace."))?;

        debug_assert!(matches!(node.node_type, NodeType::Output | NodeType::Workspace | NodeType::Dockarea));
    }
    let workspace = node;

    //
    // Traverse the tree and collect all the leaves while sorting.
    //
    let mut node = Some(Node::from(workspace));
    let mut stack = Vec::new();
    let mut _depth = 0;
    let mut windows = Vec::new();

    while node.is_some() || !stack.is_empty() {
        if let Some(mut n) = node.take() {
            if is_node_leaf!(n) {
                dbg_println!("{} visiting leaf {}: {:?}", "-".repeat(_depth+1), n.id, n.name);
                windows.push(n);
            } else {
                dbg_println!("{} found branch {}", "-".repeat(_depth), n.id);
                _depth += 1;
                dbg_println!("{} children {:?}", "-".repeat(_depth), n.nodes.iter().map(|x| x.id).collect::<Vec<_>>());

                // Sorting here ensures the windows are sorted by rect/deco_rect, while still respecting the
                // structure of the tree. Also this means we don't usually have to do much, since the windows
                // are often already in order.
                n.nodes.sort_unstable_by(Node::asc_above_or_right_of);
                dbg_println!("{} sorted children {:?}", "-".repeat(_depth), n.nodes.iter().map(|x| x.id).collect::<Vec<_>>());

                stack.push(n.nodes);
            }
        } else if let Some(mut v) = stack.pop() {
            if v.is_empty() {
                _depth -= 1;
                dbg_println!("{} consumed branch", "-".repeat(_depth));
                assert!(!windows.is_empty());
            } else {
                // Move to the next node of the current branch in the stack.
                node = v.pop();
                stack.push(v);
            }
        } else {
            unreachable!("Got no node nor a stack of containers, what's going on?")
        }
    }

    debug_assert!(node.is_none(), "{:?}", node);
    debug_assert!(stack.is_empty(), "{:?}", stack);

    // For tests:
    dbg_println!("<id>: <rect>, <deco_rect>");
    #[cfg(debug_assertions)]
    for w in &windows {
        dbg_println!("{}", w);
    }

    //
    // Construct and run a sway IPC command.
    //
    let focused_idx = windows.iter().position(|x| x.focused)
        .ok_or_else(|| anyhow!("Could not find the focused window"))?;
    dbg_dbg!(focused_idx);

    let next_idx = match cmd_arg {
        CommandArgument::Next => {
            if windows.len() > focused_idx + 1 {
                focused_idx + 1
            } else {
                0
            }
        },
        CommandArgument::Prev => {
            if focused_idx > 0 {
                focused_idx - 1
            } else {
                windows.len() - 1
            }
        }
    };

    let cmd_msg = match cmd {
        Command::Focus => format!("[con_id={}] focus", windows[next_idx].id),
        Command::Move => format!("swap container with con_id {}", windows[next_idx].id)
    };

    dbg_dbg!(&cmd_msg);
    conn.run_command(cmd_msg)?;

    Ok(())
}
