/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::default::Default;

use html5ever::local_name;
use js::context::{JSContext, NoGC};
use script_bindings::cell::DomRefCell;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::DocumentBinding::DocumentMethods;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::element::element::Element;
use crate::dom::html::interactive::htmldialogelement::HTMLDialogElement;
use crate::dom::window::Window;

/// The [`DocumentOverlayHandler`] is a structure responsible for overlays for
/// the [`crate::Document`] and storing data related to overlays. It exists to
/// decrease the size of the [`crate::Document`] structure.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) struct DocumentOverlayHandler {
    /// The [`Window`] element for this [`DocumentOverlayHandler`].
    window: Dom<Window>,
    /// Entry node for fullscreen.
    fullscreen_element: MutNullableDom<Element>,
    /// <https://drafts.csswg.org/css-position-4/#document-top-layer>
    top_layer: DomRefCell<Vec<Dom<Element>>>,
    /// <https://drafts.csswg.org/css-position-4/#document-pending-top-layer-removals>
    pending_top_layer_removals: DomRefCell<Vec<Dom<Element>>>,
    /// <https://html.spec.whatwg.org/multipage/#blocked-by-a-modal-dialog>
    topmost_modal: MutNullableDom<Element>,
    /// <https://html.spec.whatwg.org/multipage/#open-dialogs-list>
    open_dialogs_list: DomRefCell<Vec<Dom<HTMLDialogElement>>>,
    /// <https://html.spec.whatwg.org/multipage/#dialog-pointerdown-target>
    dialog_pointerdown_target: MutNullableDom<HTMLDialogElement>,
}

impl DocumentOverlayHandler {
    pub(crate) fn new(window: &Window) -> Self {
        Self {
            window: Dom::from_ref(window),
            fullscreen_element: MutNullableDom::new(None),
            top_layer: Default::default(),
            pending_top_layer_removals: Default::default(),
            topmost_modal: Default::default(),
            open_dialogs_list: Default::default(),
            dialog_pointerdown_target: Default::default(),
        }
    }

    pub(crate) fn fullscreen_element(&self) -> Option<DomRoot<Element>> {
        self.fullscreen_element.get()
    }

    pub(crate) fn set_fullscreen_element(&self, element: Option<&Element>) {
        self.fullscreen_element.set(element);
    }

    /// <https://fullscreen.spec.whatwg.org/#fully-exit-fullscreen>
    pub(crate) fn fully_exit_fullscreen(&self, cx: &mut JSContext) {
        // Step 1. If document’s fullscreen element is null, terminate these
        // steps.
        if self.fullscreen_element().is_none() {
            return;
        };

        // TODO Step 2. Unfullscreen elements whose fullscreen flag is set,
        // within document’s top layer, except for document’s fullscreen element.

        // Step 3. Exit fullscreen document.
        let _ = self.exit_fullscreen(cx);
    }

    pub(crate) fn open_dialogs_list_contains(&self, dialog: &HTMLDialogElement) -> bool {
        self.open_dialogs_list
            .borrow()
            .iter()
            .any(|open_dialog| &**open_dialog == dialog)
    }

    pub(crate) fn add_to_open_dialogs_list(&self, dialog: &HTMLDialogElement, no_gc: &NoGC) {
        self.open_dialogs_list
            .safe_borrow_mut(no_gc)
            .push(Dom::from_ref(dialog));
    }

    pub(crate) fn remove_from_open_dialogs_list(&self, dialog: &HTMLDialogElement, no_gc: &NoGC) {
        self.open_dialogs_list
            .safe_borrow_mut(no_gc)
            .retain(|open| &**open != dialog);
    }

    pub(crate) fn topmost_open_dialog(&self) -> Option<DomRoot<HTMLDialogElement>> {
        self.open_dialogs_list
            .borrow()
            .last()
            .map(|dialog| dialog.as_rooted())
    }

    pub(crate) fn dialog_pointerdown_target(&self) -> Option<DomRoot<HTMLDialogElement>> {
        self.dialog_pointerdown_target.get()
    }

    pub(crate) fn set_dialog_pointerdown_target(&self, dialog: Option<&HTMLDialogElement>) {
        self.dialog_pointerdown_target.set(dialog);
    }

    pub(crate) fn top_layer_contains(&self, element: &Element) -> bool {
        self.top_layer
            .borrow()
            .iter()
            .any(|top_layer_element| &**top_layer_element == element)
    }

    fn pending_top_layer_removals_contains(&self, element: &Element) -> bool {
        self.pending_top_layer_removals
            .borrow()
            .iter()
            .any(|pending_element| &**pending_element == element)
    }

    /// <https://drafts.csswg.org/css-position-4/#in-the-top-layer>
    pub(crate) fn is_in_the_top_layer(&self, element: &Element) -> bool {
        // > An element el is in the top layer if el is contained in its node document’s top layer
        // > but not contained in its node document’s pending top layer removals.
        self.top_layer_contains(element) && !self.pending_top_layer_removals_contains(element)
    }

    /// <https://drafts.csswg.org/css-position-4/#add-an-element-to-the-top-layer>
    pub(crate) fn add_an_element_to_the_top_layer(&self, element: &Element, no_gc: &NoGC) {
        // Step 1. Let doc be el’s node document.

        // Step 2. If el is already contained in doc's top layer:
        if self.top_layer_contains(element) {
            // Step 2.1. Assert: el is also in doc's pending top layer removals. (Otherwise, this is
            // a spec error.)
            assert!(self.pending_top_layer_removals_contains(element));

            // Step 2.2. Remove el from both doc's top layer and pending top layer removals.
            self.top_layer
                .safe_borrow_mut(no_gc)
                .retain(|top_layer_element| &**top_layer_element != element);
            self.pending_top_layer_removals
                .safe_borrow_mut(no_gc)
                .retain(|pending_element| &**pending_element != element);
        }

        // Step 3. Append el to doc's top layer.
        self.top_layer
            .safe_borrow_mut(no_gc)
            .push(Dom::from_ref(element));

        // TODO: Step 4. At the UA !important cascade origin, add a rule targeting el
        // containing an overlay: auto declaration.

        self.update_topmost_modal(no_gc);
    }

    /// <https://drafts.csswg.org/css-position-4/#request-an-element-to-be-removed-from-the-top-layer>
    pub(crate) fn request_an_element_to_be_removed_from_the_top_layer(
        &self,
        element: &Element,
        no_gc: &NoGC,
    ) {
        // Step 1. Let doc be el's node document.

        // Step 2. If el is not contained doc's top layer, or el is already contained in doc's
        // pending top layer removals, return.
        if !self.top_layer_contains(element) || self.pending_top_layer_removals_contains(element) {
            return;
        }

        // TODO: Step 3. Remove the UA !important overlay: auto rule targeting el.

        // Step 4. Append el to doc's pending top layer removals.
        self.pending_top_layer_removals
            .safe_borrow_mut(no_gc)
            .push(Dom::from_ref(element));

        self.update_topmost_modal(no_gc);
    }

    /// <https://drafts.csswg.org/css-position-4/#remove-an-element-from-the-top-layer-immediately>
    pub(crate) fn remove_an_element_from_the_top_layer_immediately(
        &self,
        element: &Element,
        no_gc: &NoGC,
    ) {
        // Step 1. Let doc be el's node document.

        // Step 2. Remove el from doc's top layer and pending top layer removals.
        self.top_layer
            .safe_borrow_mut(no_gc)
            .retain(|top_layer_element| &**top_layer_element != element);
        self.pending_top_layer_removals
            .safe_borrow_mut(no_gc)
            .retain(|pending_element| &**pending_element != element);

        // TODO: Step 3. Remove the UA !important overlay: auto rule targeting el, if it exists.

        self.update_topmost_modal(no_gc);
    }

    /// <https://drafts.csswg.org/css-position-4/#process-top-layer-removals>
    pub(crate) fn process_top_layer_removals(&self, no_gc: &NoGC) {
        // Step 1. For each element el in doc’s pending top layer removals: if el's computed value
        // of overlay is none, or el is not rendered, remove from the top layer immediately el.
        // TODO: Check `overlay` once it is supported.
        let pending_removals: Vec<_> = self
            .pending_top_layer_removals
            .borrow()
            .iter()
            .map(|element| DomRoot::from_ref(&**element))
            .collect();
        for element in pending_removals {
            if !element.has_css_layout_box() {
                self.remove_an_element_from_the_top_layer_immediately(&element, no_gc)
            }
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#blocked-by-a-modal-dialog>
    pub fn update_topmost_modal(&self, no_gc: &NoGC) {
        // > A Document document is blocked by a modal dialog subject if subject is the topmost
        // > dialog element in document's top layer.
        let new_topmost_modal = self
            .top_layer
            .borrow()
            .iter()
            .rev()
            .find(|element| {
                element.state().contains(ElementState::MODAL) &&
                    !self.pending_top_layer_removals_contains(element)
            })
            .map(|element| DomRoot::from_ref(&**element));
        let old_topmost_modal = self.topmost_modal.get();

        if let Some(old_topmost_modal) = old_topmost_modal {
            old_topmost_modal.set_state(no_gc, ElementState::TOPMOST_MODAL, false);
        }
        if let Some(new_topmost_modal) = &new_topmost_modal {
            new_topmost_modal.set_state(no_gc, ElementState::TOPMOST_MODAL, true);
        }

        self.topmost_modal.set(new_topmost_modal.as_deref());

        // > While document is so blocked, every node that is connected to document, with the
        // > exception of the subject element and its flat tree descendants, must become inert.
        if let Some(document_element) = self.GetDocumentElement() {
            let is_inert = new_topmost_modal.is_some() ||
                document_element.has_attribute(&local_name!("inert"));
            document_element.set_state(no_gc, ElementState::INERT, is_inert);
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#blocked-by-a-modal-dialog>
    pub(crate) fn is_blocked_by_a_modal_dialog(&self) -> bool {
        self.topmost_modal.get().is_some()
    }
}
