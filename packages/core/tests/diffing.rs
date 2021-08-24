//! Diffing Tests
//! -------------
//!
//! These should always compile and run, but the result is not validat root: (), m: () ed for each test.
//! TODO: Validate the results beyond visual inspection.

use bumpalo::Bump;

use anyhow::{Context, Result};
use dioxus::{
    arena::SharedResources, diff::DiffMachine, prelude::*, DiffInstruction, DomEdit, MountType,
};
use dioxus_core as dioxus;
use dioxus_html as dioxus_elements;
use futures_util::FutureExt;
mod test_logging;
use DomEdit::*;

struct TestDom {
    bump: Bump,
    resources: SharedResources,
}
impl TestDom {
    fn new() -> TestDom {
        test_logging::set_up_logging();
        let bump = Bump::new();
        let resources = SharedResources::new();
        TestDom { bump, resources }
    }
    fn new_factory<'a>(&'a self) -> NodeFactory<'a> {
        NodeFactory::new(&self.bump)
    }

    fn render<'a, F>(&'a self, lazy_nodes: LazyNodes<'a, F>) -> VNode<'a>
    where
        F: FnOnce(NodeFactory<'a>) -> VNode<'a>,
    {
        use dioxus_core::nodes::{IntoVNode, IntoVNodeList};
        lazy_nodes.into_vnode(NodeFactory::new(&self.bump))
    }

    fn diff<'a>(&'a self, old: &'a VNode<'a>, new: &'a VNode<'a>) -> Mutations<'a> {
        // let mut edits = Vec::new();
        let mut machine = DiffMachine::new_headless(&self.resources);

        machine.stack.push(DiffInstruction::DiffNode { new, old });

        machine.mutations
    }

    fn create<'a, F1>(&'a self, left: LazyNodes<'a, F1>) -> Mutations<'a>
    where
        F1: FnOnce(NodeFactory<'a>) -> VNode<'a>,
    {
        let old = self.bump.alloc(self.render(left));

        let mut machine = DiffMachine::new_headless(&self.resources);

        machine.stack.create_node(old, MountType::Append);

        work_sync(&mut machine);

        machine.mutations
    }

    fn lazy_diff<'a, F1, F2>(
        &'a self,
        left: LazyNodes<'a, F1>,
        right: LazyNodes<'a, F2>,
    ) -> (Mutations<'a>, Mutations<'a>)
    where
        F1: FnOnce(NodeFactory<'a>) -> VNode<'a>,
        F2: FnOnce(NodeFactory<'a>) -> VNode<'a>,
    {
        let old = self.bump.alloc(self.render(left));

        let new = self.bump.alloc(self.render(right));

        // let mut create_edits = Vec::new();

        let mut machine = DiffMachine::new_headless(&self.resources);

        machine.stack.create_node(old, MountType::Append);

        work_sync(&mut machine);
        let create_edits = machine.mutations;

        let mut machine = DiffMachine::new_headless(&self.resources);
        machine.stack.push(DiffInstruction::DiffNode { old, new });
        work_sync(&mut machine);
        let edits = machine.mutations;

        (create_edits, edits)
    }
}

fn work_sync(machine: &mut DiffMachine) {
    let mut fut = machine.work().boxed_local();

    while let None = (&mut fut).now_or_never() {
        //
    }
}

#[test]
fn diffing_works() {}

/// Should push the text node onto the stack and modify it
#[test]
fn html_and_rsx_generate_the_same_output() {
    let dom = TestDom::new();
    let (create, change) = dom.lazy_diff(
        rsx! ( div { "Hello world" } ),
        rsx! ( div { "Goodbye world" } ),
    );
    assert_eq!(
        create.edits,
        [
            CreateElement { id: 0, tag: "div" },
            CreateTextNode {
                id: 1,
                text: "Hello world"
            },
            AppendChildren { many: 1 },
            AppendChildren { many: 1 },
        ]
    );

    assert_eq!(
        change.edits,
        [
            PushRoot { id: 1 },
            SetText {
                text: "Goodbye world"
            },
            PopRoot
        ]
    );
}

/// Should result in 3 elements on the stack
#[test]
fn fragments_create_properly() {
    let dom = TestDom::new();

    let create = dom.create(rsx! {
        div { "Hello a" }
        div { "Hello b" }
        div { "Hello c" }
    });

    assert_eq!(
        create.edits,
        [
            CreateElement { id: 0, tag: "div" },
            CreateTextNode {
                id: 1,
                text: "Hello a"
            },
            AppendChildren { many: 1 },
            CreateElement { id: 2, tag: "div" },
            CreateTextNode {
                id: 3,
                text: "Hello b"
            },
            AppendChildren { many: 1 },
            CreateElement { id: 4, tag: "div" },
            CreateTextNode {
                id: 5,
                text: "Hello c"
            },
            AppendChildren { many: 1 },
            AppendChildren { many: 3 },
        ]
    );
}

/// Should result in the creation of an anchor (placeholder) and then a replacewith
#[test]
fn empty_fragments_create_anchors() {
    let dom = TestDom::new();

    let left = rsx!({ (0..0).map(|f| rsx! { div {}}) });
    let right = rsx!({ (0..1).map(|f| rsx! { div {}}) });

    let (create, change) = dom.lazy_diff(left, right);

    assert_eq!(
        create.edits,
        [CreatePlaceholder { id: 0 }, AppendChildren { many: 1 }]
    );
    assert_eq!(
        change.edits,
        [
            CreateElement { id: 1, tag: "div" },
            ReplaceWith { m: 1, root: 0 }
        ]
    );
}

/// Should result in the creation of an anchor (placeholder) and then a replacewith m=5
#[test]
fn empty_fragments_create_many_anchors() {
    let dom = TestDom::new();

    let left = rsx!({ (0..0).map(|f| rsx! { div {}}) });
    let right = rsx!({ (0..5).map(|f| rsx! { div {}}) });

    let (create, change) = dom.lazy_diff(left, right);
    assert_eq!(
        create.edits,
        [CreatePlaceholder { id: 0 }, AppendChildren { many: 1 }]
    );
    assert_eq!(
        change.edits,
        [
            CreateElement { id: 1, tag: "div" },
            CreateElement { id: 2, tag: "div" },
            CreateElement { id: 3, tag: "div" },
            CreateElement { id: 4, tag: "div" },
            CreateElement { id: 5, tag: "div" },
            ReplaceWith { m: 5, root: 0 }
        ]
    );
}

/// Should result in the creation of an anchor (placeholder) and then a replacewith
/// Includes child nodes inside the fragment
#[test]
fn empty_fragments_create_anchors_with_many_children() {
    let dom = TestDom::new();

    let left = rsx!({ (0..0).map(|f| rsx! { div {} }) });
    let right = rsx!({
        (0..3).map(|f| {
            rsx! { div { "hello: {f}" }}
        })
    });

    let (create, change) = dom.lazy_diff(left, right);
    assert_eq!(
        create.edits,
        [CreatePlaceholder { id: 0 }, AppendChildren { many: 1 }]
    );
    assert_eq!(
        change.edits,
        [
            CreateElement { id: 1, tag: "div" },
            CreateTextNode {
                text: "hello: 0",
                id: 2
            },
            AppendChildren { many: 1 },
            CreateElement { id: 3, tag: "div" },
            CreateTextNode {
                text: "hello: 1",
                id: 4
            },
            AppendChildren { many: 1 },
            CreateElement { id: 5, tag: "div" },
            CreateTextNode {
                text: "hello: 2",
                id: 6
            },
            AppendChildren { many: 1 },
            ReplaceWith { m: 3, root: 0 }
        ]
    );
}

/// Should result in every node being pushed and then replaced with an anchor
#[test]
fn many_items_become_fragment() {
    let dom = TestDom::new();

    let left = rsx!({
        (0..2).map(|f| {
            rsx! { div { "hello" }}
        })
    });
    let right = rsx!({ (0..0).map(|f| rsx! { div {} }) });

    let (create, change) = dom.lazy_diff(left, right);
    assert_eq!(
        create.edits,
        [
            CreateElement { id: 0, tag: "div" },
            CreateTextNode {
                text: "hello",
                id: 1
            },
            AppendChildren { many: 1 },
            CreateElement { id: 2, tag: "div" },
            CreateTextNode {
                text: "hello",
                id: 3
            },
            AppendChildren { many: 1 },
            AppendChildren { many: 2 },
        ]
    );

    // hmmmmmmmmm worried about reusing IDs that we shouldnt be
    assert_eq!(
        change.edits,
        [
            Remove { root: 2 },
            CreatePlaceholder { id: 4 },
            ReplaceWith { root: 0, m: 1 },
        ]
    );
}

/// Should result in no edits
#[test]
fn two_equal_fragments_are_equal() {
    let dom = TestDom::new();

    let left = rsx!({
        (0..2).map(|f| {
            rsx! { div { "hello" }}
        })
    });
    let right = rsx!({
        (0..2).map(|f| {
            rsx! { div { "hello" }}
        })
    });

    let (create, change) = dom.lazy_diff(left, right);
    assert!(change.edits.is_empty());
}

/// Should result the creation of more nodes appended after the old last node
#[test]
fn two_fragments_with_differrent_elements_are_differet() {
    let dom = TestDom::new();

    let left = rsx!(
        { (0..2).map(|f| rsx! { div {  }} ) }
        p {}
    );
    let right = rsx!(
        { (0..5).map(|f| rsx! (h1 {  }) ) }
        p {}
    );

    let edits = dom.lazy_diff(left, right);
    dbg!(&edits);
}

/// Should result in multiple nodes destroyed - with changes to the first nodes
#[test]
fn two_fragments_with_differrent_elements_are_differet_shorter() {
    let dom = TestDom::new();

    let left = rsx!(
        {(0..5).map(|f| {rsx! { div {  }}})}
        p {}
    );
    let right = rsx!(
        {(0..2).map(|f| {rsx! { h1 {  }}})}
        p {}
    );

    let (create, change) = dom.lazy_diff(left, right);
    assert_eq!(
        create.edits,
        [
            CreateElement { id: 0, tag: "div" },
            CreateElement { id: 1, tag: "div" },
            CreateElement { id: 2, tag: "div" },
            CreateElement { id: 3, tag: "div" },
            CreateElement { id: 4, tag: "div" },
            CreateElement { id: 5, tag: "p" },
            AppendChildren { many: 6 },
        ]
    );
    assert_eq!(
        change.edits,
        [
            Remove { root: 2 },
            Remove { root: 3 },
            Remove { root: 4 },
            CreateElement { id: 6, tag: "h1" },
            ReplaceWith { root: 0, m: 1 },
            CreateElement { id: 7, tag: "h1" },
            ReplaceWith { root: 1, m: 1 },
        ]
    );
}

/// Should result in multiple nodes destroyed - with no changes
#[test]
fn two_fragments_with_same_elements_are_differet() {
    let dom = TestDom::new();

    let left = rsx!(
        {(0..2).map(|f| {rsx! { div {  }}})}
        p {}
    );
    let right = rsx!(
        {(0..5).map(|f| {rsx! { div {  }}})}
        p {}
    );

    let (create, change) = dom.lazy_diff(left, right);
    assert_eq!(
        create.edits,
        [
            CreateElement { id: 0, tag: "div" },
            CreateElement { id: 1, tag: "div" },
            CreateElement { id: 2, tag: "p" },
            AppendChildren { many: 3 },
        ]
    );
    assert_eq!(
        change.edits,
        [
            CreateElement { id: 3, tag: "div" },
            CreateElement { id: 4, tag: "div" },
            CreateElement { id: 5, tag: "div" },
            InsertAfter { root: 1, n: 3 },
        ]
    );
}

/// should result in the removal of elements
#[test]
fn keyed_diffing_order() {
    let dom = TestDom::new();

    let left = rsx!(
        {(0..5).map(|f| {rsx! { div { key: "{f}"  }}})}
        p {"e"}
    );
    let right = rsx!(
        {(0..2).map(|f| {rsx! { div { key: "{f}" }}})}
        p {"e"}
    );

    let (create, change) = dom.lazy_diff(left, right);
    assert_eq!(
        change.edits,
        [Remove { root: 2 }, Remove { root: 3 }, Remove { root: 4 },]
    );
}

#[test]
fn fragment_keys() {
    let r = 1;
    let p = rsx! {
        Fragment { key: "asd {r}" }
    };
}

/// Should result in moves, but not removals or additions
#[test]
fn keyed_diffing_out_of_order() {
    let dom = TestDom::new();

    let left = rsx!({
        [0, 1, 2, 3, /**/ 4, 5, 6, /**/ 7, 8, 9].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let right = rsx!({
        [0, 1, 2, 3, /**/ 6, 4, 5, /**/ 7, 8, 9].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let edits = dom.lazy_diff(left, right);
    dbg!(&edits.1);
}

/// Should result in moves only
#[test]
fn keyed_diffing_out_of_order_adds() {
    let dom = TestDom::new();

    let left = rsx!({
        [/**/ 4, 5, 6, 7, 8 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let right = rsx!({
        [/**/ 8, 7, 4, 5, 6 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let (_, change) = dom.lazy_diff(left, right);
    assert_eq!(
        change.edits,
        [
            PushRoot { id: 4 },
            PushRoot { id: 3 },
            InsertBefore { n: 2, root: 0 }
        ]
    );
}
/// Should result in moves onl
#[test]
fn keyed_diffing_out_of_order_adds_2() {
    let dom = TestDom::new();

    let left = rsx!({
        [/**/ 4, 5, 6, 7, 8 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let right = rsx!({
        [/**/ 7, 8, 4, 5, 6 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let (_, change) = dom.lazy_diff(left, right);
    assert_eq!(
        change.edits,
        [
            PushRoot { id: 3 },
            PushRoot { id: 4 },
            InsertBefore { n: 2, root: 0 }
        ]
    );
}

/// Should result in moves onl
#[test]
fn keyed_diffing_out_of_order_adds_3() {
    let dom = TestDom::new();

    let left = rsx!({
        [/**/ 4, 5, 6, 7, 8 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let right = rsx!({
        [/**/ 4, 8, 7, 5, 6 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let (_, change) = dom.lazy_diff(left, right);
    assert_eq!(
        change.edits,
        [
            PushRoot { id: 4 },
            PushRoot { id: 3 },
            InsertBefore { n: 2, root: 1 }
        ]
    );
}

/// Should result in moves onl
#[test]
fn keyed_diffing_out_of_order_adds_4() {
    let dom = TestDom::new();

    let left = rsx!({
        [/**/ 4, 5, 6, 7, 8 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let right = rsx!({
        [/**/ 4, 5, 8, 7, 6 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let (_, change) = dom.lazy_diff(left, right);
    assert_eq!(
        change.edits,
        [
            PushRoot { id: 4 },
            PushRoot { id: 3 },
            InsertBefore { n: 2, root: 2 }
        ]
    );
}

/// Should result in moves onl
#[test]
fn keyed_diffing_out_of_order_adds_5() {
    let dom = TestDom::new();

    let left = rsx!({
        [/**/ 4, 5, 6, 7, 8 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let right = rsx!({
        [/**/ 4, 5, 6, 8, 7 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let (_, change) = dom.lazy_diff(left, right);
    assert_eq!(
        change.edits,
        [PushRoot { id: 4 }, InsertBefore { n: 1, root: 3 }]
    );
}

#[test]
fn keyed_diffing_additions() {
    let dom = TestDom::new();

    let left = rsx!({
        [/**/ 4, 5, 6, 7, 8 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let right = rsx!({
        [/**/ 4, 5, 6, 7, 8, 9, 10 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let (_, change) = dom.lazy_diff(left, right);
    assert_eq!(
        change.edits,
        [
            CreateElement { id: 5, tag: "div" },
            CreateElement { id: 6, tag: "div" },
            InsertAfter { n: 2, root: 4 }
        ]
    );
}

#[test]
fn keyed_diffing_additions_and_moves_on_ends() {
    let dom = TestDom::new();

    let left = rsx!({
        [/**/ 4, 5, 6, 7 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let right = rsx!({
        [/**/ 7, 4, 5, 6, 11, 12 /**/].iter().map(|f| {
            // [/**/ 8, 7, 4, 5, 6, 9, 10 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let (_, change) = dom.lazy_diff(left, right);
    dbg!(change);
    // assert_eq!(
    //     change.edits,
    //     [
    //         CreateElement { id: 5, tag: "div" },
    //         CreateElement { id: 6, tag: "div" },
    //         InsertAfter { n: 2, root: 4 }
    //     ]
    // );
}

#[test]
fn keyed_diffing_additions_and_moves_in_middle() {
    let dom = TestDom::new();

    let left = rsx!({
        [/**/ 4, 5, 6, 7 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let right = rsx!({
        [/**/ 7, 4, 13, 17, 5, 11, 12, 6 /**/].iter().map(|f| {
            rsx! { div { key: "{f}"  }}
        })
    });

    let (_, change) = dom.lazy_diff(left, right);
    dbg!(change);
    // assert_eq!(
    //     change.edits,
    //     [
    //         CreateElement { id: 5, tag: "div" },
    //         CreateElement { id: 6, tag: "div" },
    //         InsertAfter { n: 2, root: 4 }
    //     ]
    // );
}

#[test]
fn controlled_keyed_diffing_out_of_order() {
    let dom = TestDom::new();

    let left = [4, 5, 6, 7];
    let left = rsx!({
        left.iter().map(|f| {
            rsx! { div { key: "{f}" "{f}" }}
        })
    });

    // 0, 1, 2, 6, 5, 4, 3, 7, 8, 9
    let right = [0, 5, 9, 6, 4];
    let right = rsx!({
        right.iter().map(|f| {
            rsx! { div { key: "{f}" "{f}" }}
        })
    });

    // LIS: 3, 7, 8,
    let edits = dom.lazy_diff(left, right);
    dbg!(&edits);
}
