use super::*;
use crate::app::platform;
use egui_dock::{Node, Tree};

const FILE: &str = "editor_workspaces.json";

#[derive(Serialize, Deserialize)]
struct Layouts {
    version: u32,
    layouts: [Option<SavedNode>; 3],
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum SavedNode {
    Pane(PaneKind),
    Split {
        vertical: bool,
        fraction: f32,
        first: Box<SavedNode>,
        second: Box<SavedNode>,
    },
}

impl SavedNode {
    fn capture(tree: &Tree<Pane>, at: NodeIndex) -> Option<Self> {
        match &tree[at] {
            Node::Leaf { tabs, .. } => tabs.first().map(|pane| Self::Pane(pane.kind)),
            Node::Vertical { fraction, .. } | Node::Horizontal { fraction, .. } => {
                Some(Self::Split {
                    vertical: matches!(tree[at], Node::Vertical { .. }),
                    fraction: *fraction,
                    first: Box::new(Self::capture(tree, NodeIndex(at.0 * 2 + 1))?),
                    second: Box::new(Self::capture(tree, NodeIndex(at.0 * 2 + 2))?),
                })
            }
            Node::Empty => None,
        }
    }

    fn valid(&self, seen: &mut Vec<PaneKind>, depth: usize) -> bool {
        if depth > PaneKind::ALL.len() {
            return false;
        }
        match self {
            Self::Pane(kind) => {
                if seen.contains(kind) {
                    return false;
                }
                seen.push(*kind);
                true
            }
            Self::Split {
                fraction,
                first,
                second,
                ..
            } => {
                fraction.is_finite()
                    && *fraction > 0.0
                    && *fraction < 1.0
                    && first.valid(seen, depth + 1)
                    && second.valid(seen, depth + 1)
            }
        }
    }

    fn first(&self) -> PaneKind {
        match self {
            Self::Pane(kind) => *kind,
            Self::Split { first, .. } => first.first(),
        }
    }

    fn fill(&self, tree: &mut Tree<Pane>, at: NodeIndex, next_id: &mut u64) {
        if let Self::Split {
            vertical,
            fraction,
            first,
            second,
        } = self
        {
            *next_id += 1;
            let [a, b] = tree.split_tabs(
                at,
                if *vertical {
                    Split::Below
                } else {
                    Split::Right
                },
                *fraction,
                vec![Pane {
                    id: *next_id,
                    kind: second.first(),
                }],
            );
            first.fill(tree, a, next_id);
            second.fill(tree, b, next_id);
        }
    }

    fn restore(&self, next_id: &mut u64) -> DockState<Pane> {
        *next_id += 1;
        let mut dock = DockState::new(vec![Pane {
            id: *next_id,
            kind: self.first(),
        }]);
        self.fill(dock.main_surface_mut(), NodeIndex::root(), next_id);
        dock
    }
}

impl Workbench {
    pub(super) fn load_layouts(&mut self) {
        let result = (|| -> Result<Option<Layouts>, String> {
            let path = platform::output_dir()?.join(FILE);
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error.to_string()),
            };
            let parsed = serde_json::from_str::<Layouts>(&text).ok().filter(|saved| {
                saved.version == 1
                    && saved
                        .layouts
                        .iter()
                        .flatten()
                        .all(|node| node.valid(&mut Vec::new(), 0))
            });
            if let Some(saved) = parsed {
                return Ok(Some(saved));
            }
            let backup = path.with_extension("invalid.json");
            std::fs::copy(&path, &backup).map_err(|e| e.to_string())?;
            Err(format!(
                "Invalid workspace layout; backup: {}",
                backup.display()
            ))
        })();
        match result {
            Ok(Some(saved)) => {
                for (index, node) in saved.layouts.into_iter().enumerate() {
                    if let Some(node) = node {
                        let dock = node.restore(&mut self.next_id);
                        self.saved[index] = Some(dock.clone());
                        self.docks[index] = dock;
                    }
                }
            }
            Ok(None) => {}
            Err(error) => self.message = Some(error),
        }
    }

    fn save_layouts(&self) -> Result<(), String> {
        let saved = Layouts {
            version: 1,
            layouts: std::array::from_fn(|i| {
                self.saved[i]
                    .as_ref()
                    .and_then(|dock| SavedNode::capture(dock.main_surface(), NodeIndex::root()))
            }),
        };
        let text = serde_json::to_string_pretty(&saved).map_err(|e| e.to_string())?;
        let temp = platform::write_output_text("editor_workspaces.tmp", &text)?;
        std::fs::rename(&temp, temp.with_file_name(FILE)).map_err(|e| e.to_string())
    }

    pub fn remember_layout(&mut self) {
        self.saved[self.workspace] = Some(self.docks[self.workspace].clone());
        self.message = Some(match self.save_layouts() {
            Ok(()) => "布局已保存".into(),
            Err(error) => format!("布局保存失败: {error}"),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout_roundtrip_keeps_order_and_proportions() {
        let mut next = 0;
        for index in 0..3 {
            let dock = preset(index, &mut next);
            let saved = SavedNode::capture(dock.main_surface(), NodeIndex::root()).unwrap();
            assert!(saved.valid(&mut Vec::new(), 0));
            let restored = saved.restore(&mut next);
            assert_eq!(
                Some(saved),
                SavedNode::capture(restored.main_surface(), NodeIndex::root())
            );
        }
    }
}
