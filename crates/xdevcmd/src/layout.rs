//! A binary split tree, the way tmux and VS Code arrange panes.
//!
//! Kept separate from rendering so the geometry is testable: "split the focused pane,
//! then close it, and the layout is what it was" is exactly the kind of thing that breaks
//! silently and is miserable to debug through a screenshot.

use ratatui::layout::{Constraint, Layout, Rect};

pub type PaneId = u32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    /// Children sit side by side.
    Cols,
    /// Children sit one above the other.
    Rows,
}

#[derive(Debug, Clone)]
pub enum Node {
    Leaf(PaneId),
    Split {
        axis: Axis,
        /// Percentage of the area given to `a`.
        pct: u16,
        a: Box<Node>,
        b: Box<Node>,
    },
}

impl Node {
    pub fn split(axis: Axis, pct: u16, a: Node, b: Node) -> Self {
        Node::Split {
            axis,
            pct,
            a: Box::new(a),
            b: Box::new(b),
        }
    }

    /// Walk the tree and hand every leaf its rectangle.
    pub fn assign(&self, area: Rect, out: &mut Vec<(PaneId, Rect)>) {
        match self {
            Node::Leaf(id) => out.push((*id, area)),
            Node::Split { axis, pct, a, b } => {
                let pct = (*pct).clamp(10, 90);
                let c = [
                    Constraint::Percentage(pct),
                    Constraint::Percentage(100 - pct),
                ];
                let parts = match axis {
                    Axis::Cols => Layout::horizontal(c).split(area),
                    Axis::Rows => Layout::vertical(c).split(area),
                };
                a.assign(parts[0], out);
                b.assign(parts[1], out);
            }
        }
    }

    pub fn leaves(&self, out: &mut Vec<PaneId>) {
        match self {
            Node::Leaf(id) => out.push(*id),
            Node::Split { a, b, .. } => {
                a.leaves(out);
                b.leaves(out);
            }
        }
    }

    /// Replace `target` with a split holding `target` and `new_id`.
    pub fn split_leaf(&mut self, target: PaneId, axis: Axis, new_id: PaneId) -> bool {
        match self {
            Node::Leaf(id) if *id == target => {
                *self = Node::split(axis, 50, Node::Leaf(target), Node::Leaf(new_id));
                true
            }
            Node::Leaf(_) => false,
            Node::Split { a, b, .. } => {
                a.split_leaf(target, axis, new_id) || b.split_leaf(target, axis, new_id)
            }
        }
    }

    /// Remove `target`, collapsing its parent split into the surviving sibling.
    /// Refuses to remove the last remaining pane — a layout with no panes has nothing to
    /// draw and no way back.
    pub fn remove_leaf(&mut self, target: PaneId) -> bool {
        let (take_a, take_b) = match self {
            Node::Leaf(_) => return false,
            Node::Split { a, b, .. } => (
                matches!(**b, Node::Leaf(id) if id == target),
                matches!(**a, Node::Leaf(id) if id == target),
            ),
        };
        if take_a || take_b {
            if let Node::Split { a, b, .. } = self {
                let survivor = if take_a {
                    std::mem::replace(a.as_mut(), Node::Leaf(target))
                } else {
                    std::mem::replace(b.as_mut(), Node::Leaf(target))
                };
                *self = survivor;
                return true;
            }
        }
        match self {
            Node::Split { a, b, .. } => a.remove_leaf(target) || b.remove_leaf(target),
            Node::Leaf(_) => false,
        }
    }

    pub fn contains(&self, target: PaneId) -> bool {
        match self {
            Node::Leaf(id) => *id == target,
            Node::Split { a, b, .. } => a.contains(target) || b.contains(target),
        }
    }

    /// Move the boundary nearest `target` along `axis` by `delta` percent: positive is
    /// right for `Cols`, down for `Rows`. "Nearest" means the deepest split of that
    /// axis on the path to the pane, which is the border you would drag with the mouse.
    ///
    /// The boundary moves in the arrow's direction whichever side of it the pane is
    /// on, as dragging does: Alt+Shift+Up on a terminal at the bottom raises its top
    /// edge, so it grows; on a pane at the top it raises the bottom edge, so it shrinks.
    pub fn move_boundary(&mut self, target: PaneId, axis: Axis, delta: i16) -> bool {
        let Node::Split {
            axis: here,
            pct,
            a,
            b,
        } = self
        else {
            return false;
        };
        let child = if a.contains(target) {
            a
        } else if b.contains(target) {
            b
        } else {
            return false;
        };
        if child.move_boundary(target, axis, delta) {
            return true;
        }
        if *here == axis {
            *pct = (*pct as i16 + delta).clamp(10, 90) as u16;
            return true;
        }
        false
    }

    /// Every boundary between two children, with the strip of cells that grabs it.
    ///
    /// Panes draw their own borders, so the boundary is two cells wide: the right edge
    /// of one pane and the left edge of the next. Both grab it — a one-cell target on
    /// a border you can see as two lines is a target you keep missing.
    pub fn dividers(&self, area: Rect, out: &mut Vec<Divider>) {
        let mut n = 0;
        self.dividers_inner(area, out, &mut n);
    }

    fn dividers_inner(&self, area: Rect, out: &mut Vec<Divider>, n: &mut usize) {
        let Node::Split { axis, pct, a, b } = self else {
            return;
        };
        let id = *n;
        *n += 1;
        let pct = (*pct).clamp(10, 90);
        let c = [
            Constraint::Percentage(pct),
            Constraint::Percentage(100 - pct),
        ];
        let parts = match axis {
            Axis::Cols => Layout::horizontal(c).split(area),
            Axis::Rows => Layout::vertical(c).split(area),
        };
        let hit = match axis {
            Axis::Cols => Rect::new(
                (parts[0].x + parts[0].width).saturating_sub(1),
                area.y,
                2,
                area.height,
            ),
            Axis::Rows => Rect::new(
                area.x,
                (parts[0].y + parts[0].height).saturating_sub(1),
                area.width,
                2,
            ),
        };
        out.push(Divider {
            id,
            axis: *axis,
            hit,
            parent: area,
        });
        a.dividers_inner(parts[0], out, n);
        b.dividers_inner(parts[1], out, n);
    }

    /// Share the boundary numbered `id` evenly between the two panes on either side of
    /// it, leaving every other pane along that line as it was. With three panes side by
    /// side, a double-click between the first two makes those two the same width and
    /// does not touch the third. "A pane" here is whatever sits whole against the
    /// boundary: a stack of panes split the other way counts as one.
    pub fn even(&mut self, id: usize) -> bool {
        let mut n = 0;
        self.even_inner(id, &mut n)
    }

    fn even_inner(&mut self, id: usize, n: &mut usize) -> bool {
        let Node::Split { axis, a, b, .. } = self else {
            return false;
        };
        if *n != id {
            *n += 1;
            return a.even_inner(id, n) || b.even_inner(id, n);
        }
        let axis = *axis;
        // The pieces along this line, as shares of the split's extent: the same-axis
        // splits flattened, each piece a pane or a stack.
        let mut sizes = Vec::new();
        self.pieces(axis, 1.0, &mut sizes);
        let Node::Split { a, .. } = &*self else {
            return false;
        };
        let left = a.count_pieces(axis);
        let (i, j) = (left - 1, left);
        let mean = (sizes[i] + sizes[j]) / 2.0;
        sizes[i] = mean;
        sizes[j] = mean;
        self.set_pieces(axis, &sizes, &mut 0);
        true
    }

    /// The pieces along `axis` under this node, each as its share of `share`.
    fn pieces(&self, axis: Axis, share: f64, out: &mut Vec<f64>) {
        match self {
            Node::Split {
                axis: ax,
                pct,
                a,
                b,
                ..
            } if *ax == axis => {
                let p = f64::from((*pct).clamp(10, 90)) / 100.0;
                a.pieces(axis, share * p, out);
                b.pieces(axis, share * (1.0 - p), out);
            }
            _ => out.push(share),
        }
    }

    fn count_pieces(&self, axis: Axis) -> usize {
        match self {
            Node::Split { axis: ax, a, b, .. } if *ax == axis => {
                a.count_pieces(axis) + b.count_pieces(axis)
            }
            _ => 1,
        }
    }

    /// Set the same-axis splits under this node so that its pieces have `sizes`, read
    /// from `at`; returns the total of the pieces it used.
    fn set_pieces(&mut self, axis: Axis, sizes: &[f64], at: &mut usize) -> f64 {
        match self {
            Node::Split {
                axis: ax,
                pct,
                a,
                b,
                ..
            } if *ax == axis => {
                let sa = a.set_pieces(axis, sizes, at);
                let sb = b.set_pieces(axis, sizes, at);
                if sa + sb > 0.0 {
                    *pct = ((sa * 100.0 / (sa + sb)).round() as u16).clamp(10, 90);
                }
                sa + sb
            }
            _ => {
                let v = sizes[*at];
                *at += 1;
                v
            }
        }
    }

    /// Set the split numbered `id` (pre-order, as [`Node::dividers`] numbers them).
    pub fn set_pct(&mut self, id: usize, pct: u16) -> bool {
        let mut n = 0;
        self.set_pct_inner(id, pct.clamp(10, 90), &mut n)
    }

    fn set_pct_inner(&mut self, id: usize, value: u16, n: &mut usize) -> bool {
        let Node::Split { pct, a, b, .. } = self else {
            return false;
        };
        if *n == id {
            *pct = value;
            return true;
        }
        *n += 1;
        a.set_pct_inner(id, value, n) || b.set_pct_inner(id, value, n)
    }
}

/// A boundary that can be dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Divider {
    pub id: usize,
    pub axis: Axis,
    pub hit: Rect,
    /// The area the split divides; a pointer position becomes a percentage of this.
    pub parent: Rect,
}

impl Divider {
    /// The split percentage that puts the boundary under the pointer.
    pub fn pct_at(&self, x: u16, y: u16) -> u16 {
        let (pos, start, len) = match self.axis {
            Axis::Cols => (x, self.parent.x, self.parent.width),
            Axis::Rows => (y, self.parent.y, self.parent.height),
        };
        if len == 0 {
            return 50;
        }
        let rel = pos.saturating_sub(start) as u32 + 1;
        ((rel * 100 / len as u32) as u16).clamp(10, 90)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widths(n: &Node, w: u16) -> Vec<u16> {
        let mut out = Vec::new();
        n.assign(Rect::new(0, 0, w, 10), &mut out);
        out.iter().map(|(_, r)| r.width).collect()
    }

    #[test]
    fn evening_a_boundary_shares_only_its_two_panes() {
        // Two panes: 70/30 becomes 50/50.
        let mut n = Node::split(Axis::Cols, 70, Node::Leaf(1), Node::Leaf(2));
        assert!(n.even(0));
        assert_eq!(widths(&n, 100), [50, 50]);
        // Three side by side, A | (B | C) at 20 | 60 | 20: evening A|B makes A and B
        // the same and leaves C alone.
        let mut n = Node::split(
            Axis::Cols,
            20,
            Node::Leaf(1),
            Node::split(Axis::Cols, 75, Node::Leaf(2), Node::Leaf(3)),
        );
        assert_eq!(widths(&n, 100), [20, 60, 20]);
        assert!(n.even(0));
        assert_eq!(widths(&n, 100), [40, 40, 20]);
        // The inner boundary, B|C, leaves A alone.
        let mut n = Node::split(
            Axis::Cols,
            20,
            Node::Leaf(1),
            Node::split(Axis::Cols, 75, Node::Leaf(2), Node::Leaf(3)),
        );
        assert!(n.even(1));
        assert_eq!(widths(&n, 100), [20, 40, 40]);
        // (A | B) | C: the outer boundary is B|C; A keeps its width.
        let mut n = Node::split(
            Axis::Cols,
            80,
            Node::split(Axis::Cols, 25, Node::Leaf(1), Node::Leaf(2)),
            Node::Leaf(3),
        );
        assert_eq!(widths(&n, 100), [20, 60, 20]);
        assert!(n.even(0));
        assert_eq!(widths(&n, 100), [20, 40, 40]);
        // A stack split the other way counts as one piece.
        let mut n = Node::split(
            Axis::Cols,
            30,
            Node::Leaf(1),
            Node::split(Axis::Rows, 50, Node::Leaf(2), Node::Leaf(3)),
        );
        assert!(n.even(0));
        assert_eq!(widths(&n, 100), [50, 50, 50]);
        assert!(!n.even(5), "no such boundary");
    }

    fn leaves_of(n: &Node) -> Vec<PaneId> {
        let mut v = Vec::new();
        n.leaves(&mut v);
        v
    }

    #[test]
    fn a_single_leaf_takes_the_whole_area() {
        let n = Node::Leaf(7);
        let mut out = Vec::new();
        n.assign(Rect::new(0, 0, 80, 24), &mut out);
        assert_eq!(out, vec![(7, Rect::new(0, 0, 80, 24))]);
    }

    #[test]
    fn a_column_split_divides_width() {
        let n = Node::split(Axis::Cols, 50, Node::Leaf(0), Node::Leaf(1));
        let mut out = Vec::new();
        n.assign(Rect::new(0, 0, 80, 24), &mut out);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].1.width + out[1].1.width, 80);
        assert_eq!(out[0].1.height, 24);
    }

    #[test]
    fn splitting_then_closing_restores_the_layout() {
        let mut n = Node::split(Axis::Cols, 50, Node::Leaf(0), Node::Leaf(1));
        let before = leaves_of(&n);

        assert!(n.split_leaf(1, Axis::Rows, 2));
        assert_eq!(leaves_of(&n), vec![0, 1, 2]);

        assert!(n.remove_leaf(2));
        assert_eq!(leaves_of(&n), before);
    }

    #[test]
    fn closing_collapses_the_parent_split() {
        let mut n = Node::split(Axis::Cols, 50, Node::Leaf(0), Node::Leaf(1));
        assert!(n.remove_leaf(0));
        assert!(matches!(n, Node::Leaf(_)));
        assert_eq!(leaves_of(&n), vec![1]);
    }

    #[test]
    fn the_last_pane_cannot_be_closed() {
        let mut n = Node::Leaf(0);
        assert!(!n.remove_leaf(0));
        assert_eq!(leaves_of(&n), vec![0]);
    }

    fn pct_of(n: &Node) -> u16 {
        match n {
            Node::Split { pct, .. } => *pct,
            Node::Leaf(_) => 0,
        }
    }

    /// Panes 0 | 1 side by side, terminal 2 underneath: the default layout.
    fn default_layout() -> Node {
        Node::split(
            Axis::Rows,
            70,
            Node::split(Axis::Cols, 50, Node::Leaf(0), Node::Leaf(1)),
            Node::Leaf(2),
        )
    }

    #[test]
    fn the_boundary_moves_the_way_the_arrow_points_from_either_side() {
        let mut n = default_layout();
        // Right from the left pane and from the right pane both move the same border
        // right: the left pane grows in the first case, the right pane shrinks in the
        // second. That is what dragging the border would do.
        assert!(n.move_boundary(0, Axis::Cols, 5));
        assert!(n.move_boundary(1, Axis::Cols, 5));
        match &n {
            Node::Split { a, .. } => assert_eq!(pct_of(a), 60),
            _ => unreachable!(),
        }
    }

    #[test]
    fn up_on_the_terminal_raises_its_top_edge() {
        let mut n = default_layout();
        assert!(n.move_boundary(2, Axis::Rows, -10));
        assert_eq!(pct_of(&n), 60, "panes shorter, terminal taller");
        assert!(n.move_boundary(0, Axis::Rows, 20));
        assert_eq!(
            pct_of(&n),
            80,
            "from a top pane, down moves the same edge down"
        );
    }

    #[test]
    fn the_deepest_matching_split_is_the_one_moved() {
        // 0 | (1 over 3), terminal 2 below. Up from pane 1 moves the split inside the
        // right column, not the terminal's edge further out.
        let mut n = Node::split(
            Axis::Rows,
            70,
            Node::split(
                Axis::Cols,
                50,
                Node::Leaf(0),
                Node::split(Axis::Rows, 50, Node::Leaf(1), Node::Leaf(3)),
            ),
            Node::Leaf(2),
        );
        assert!(n.move_boundary(1, Axis::Rows, -10));
        assert_eq!(pct_of(&n), 70, "the outer edge stays");
        match &n {
            Node::Split { a, .. } => match a.as_ref() {
                Node::Split { b, .. } => assert_eq!(pct_of(b), 40),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    #[test]
    fn no_boundary_on_that_axis_means_nothing_moves() {
        let mut n = Node::split(Axis::Cols, 50, Node::Leaf(0), Node::Leaf(1));
        assert!(!n.move_boundary(0, Axis::Rows, 5));
        assert!(!Node::Leaf(0).move_boundary(0, Axis::Cols, 5));
        assert_eq!(pct_of(&n), 50);
    }

    #[test]
    fn dividers_sit_on_the_boundary_and_span_it() {
        // Two panes over a terminal, the default layout.
        let n = Node::split(
            Axis::Rows,
            70,
            Node::split(Axis::Cols, 50, Node::Leaf(0), Node::Leaf(1)),
            Node::Leaf(2),
        );
        let mut d = Vec::new();
        n.dividers(Rect::new(0, 0, 100, 40), &mut d);
        assert_eq!(d.len(), 2);

        let rows = d.iter().find(|d| d.axis == Axis::Rows).unwrap();
        assert_eq!(
            rows.hit.width, 100,
            "the terminal's top edge spans the screen"
        );
        assert_eq!(rows.hit.height, 2, "both border rows grab it");

        let cols = d.iter().find(|d| d.axis == Axis::Cols).unwrap();
        assert_eq!(cols.hit.width, 2);
        assert_eq!(
            cols.hit.x, 49,
            "the left pane's right edge and the right pane's left edge"
        );
        assert!(cols.hit.y + cols.hit.height <= rows.hit.y + 2);
    }

    #[test]
    fn dragging_a_divider_sets_its_split() {
        let mut n = Node::split(
            Axis::Rows,
            70,
            Node::split(Axis::Cols, 50, Node::Leaf(0), Node::Leaf(1)),
            Node::Leaf(2),
        );
        let mut d = Vec::new();
        n.dividers(Rect::new(0, 0, 100, 40), &mut d);
        let cols = *d.iter().find(|d| d.axis == Axis::Cols).unwrap();

        let pct = cols.pct_at(29, 5);
        assert_eq!(pct, 30);
        assert!(n.set_pct(cols.id, pct));

        let mut rects = Vec::new();
        n.assign(Rect::new(0, 0, 100, 40), &mut rects);
        assert_eq!(rects[0].1.width, 30, "the left pane follows the pointer");
        assert_eq!(rects[2].1.height, 12, "and the terminal split is untouched");
    }

    #[test]
    fn a_divider_cannot_be_dragged_off_the_edge() {
        let d = Divider {
            id: 0,
            axis: Axis::Rows,
            hit: Rect::ZERO,
            parent: Rect::new(0, 0, 100, 40),
        };
        assert_eq!(d.pct_at(0, 0), 10);
        assert_eq!(d.pct_at(0, 39), 90);
    }

    #[test]
    fn moving_a_boundary_stays_in_bounds() {
        let mut n = Node::split(Axis::Cols, 50, Node::Leaf(0), Node::Leaf(1));
        for _ in 0..20 {
            n.move_boundary(0, Axis::Cols, 10);
        }
        assert_eq!(pct_of(&n), 90, "must clamp, never exceed");
        for _ in 0..20 {
            n.move_boundary(0, Axis::Cols, -10);
        }
        assert_eq!(pct_of(&n), 10);
    }
}
