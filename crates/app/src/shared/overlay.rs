//! A live overlay: indicators that move for a whole agent turn (pulses,
//! shimmers, spinners, the composer's orbit) drawn on a layer of their own
//! instead of inside the view they belong to.
//!
//! Every frame an indicator asks for redraws the view it is drawn in and
//! that view's ancestors, and a cached view that redraws rebuilds every
//! view inside it. Drawn in place, a pulsing dot in the conversation would
//! rebuild the whole agent pane thirty times a second. Instead the view
//! lays out a still, invisible copy that holds the indicator's place
//! ([`LiveSlots::slot`]) and records where it landed; the [`LiveLayer`],
//! a small view drawn right after the owner (outside it, over its bounds),
//! draws the moving indicator there. The layer's frames redraw the layer
//! and the views above it, and the owner is reused from its cache.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::*;

use crate::shared::ui;

/// Builds an indicator: still in its placeholder, moving on the layer.
type Build = Rc<dyn Fn() -> AnyElement>;

/// Where an indicator sits and how to draw it.
struct Slot {
    bounds: Bounds<Pixels>,
    /// The content mask its placeholder was drawn under (a scrolled list's
    /// edge, say): the indicator is clipped to it too.
    clip: Bounds<Pixels>,
    build: Build,
}

#[derive(Default)]
struct Registry {
    /// By the view that placed them and their key.
    slots: HashMap<(EntityId, ElementId), Slot>,
    /// What floats over the owner's content (an island): indicators under
    /// it aren't drawn, as their placeholders are covered.
    covers: Vec<(EntityId, Bounds<Pixels>)>,
}

/// The indicators placed this frame. Each view clears its own as it
/// renders and places them again; a view reused from its cache keeps
/// them, since its layout didn't change.
#[derive(Clone, Default)]
pub(crate) struct LiveSlots(Rc<RefCell<Registry>>);

impl LiveSlots {
    /// Forget the indicators and covers `owner` placed: it renders again.
    pub(crate) fn clear(&self, owner: EntityId) {
        let mut registry = self.0.borrow_mut();
        registry.slots.retain(|(view, _), _| *view != owner);
        registry.covers.retain(|(view, _)| *view != owner);
    }

    /// A placeholder for the indicator `build` makes, keyed by `key` among
    /// the view's indicators: a still, invisible copy laid out in its
    /// place, while the [`LiveLayer`] draws the moving one over it. Style
    /// the placeholder as the indicator would be placed (absolute insets,
    /// say); `build` sets its own text style, since the layer doesn't
    /// inherit the place's.
    pub(crate) fn slot(
        &self,
        key: impl Into<ElementId>,
        build: impl Fn() -> AnyElement + 'static,
    ) -> Div {
        let held = ui::still(&build);
        self.slot_with_placeholder(key, held, build)
    }

    /// Use an explicitly non-animated placeholder for elements that cannot
    /// opt into `ui::still`, such as GPUI's own animations and animated images.
    pub(crate) fn slot_with_placeholder(
        &self,
        key: impl Into<ElementId>,
        placeholder: impl IntoElement,
        build: impl Fn() -> AnyElement + 'static,
    ) -> Div {
        let key = key.into();
        let build: Build = Rc::new(build);
        let slots = self.clone();
        div().relative().invisible().child(placeholder).child(
            canvas(
                move |bounds, window, _| {
                    let slot = Slot {
                        bounds,
                        clip: window.content_mask().bounds,
                        build,
                    };
                    let owner = window.current_view();
                    slots.0.borrow_mut().slots.insert((owner, key), slot);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
    }

    /// Put in something that floats over the view's content (an island,
    /// absolute): indicators it covers aren't drawn.
    pub(crate) fn cover(&self) -> impl IntoElement {
        let slots = self.clone();
        canvas(
            move |bounds, window, _| {
                let owner = window.current_view();
                slots.0.borrow_mut().covers.push((owner, bounds));
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
    }
}

/// Draws the indicators in [`LiveSlots`] where their placeholders are. The
/// view that places them hands it out; whoever draws that view draws the
/// layer right after it, over its bounds. Not cached, and it takes no
/// mouse.
pub(crate) struct LiveLayer {
    slots: LiveSlots,
}

impl LiveLayer {
    pub(crate) fn new(slots: LiveSlots) -> Self {
        Self { slots }
    }
}

impl Render for LiveLayer {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        crate::shared::diagnostics::render("LiveLayer");
        div()
            .absolute()
            .inset_0()
            .child(Indicators(self.slots.clone()))
    }
}

/// The layer's element: the slots are read in prepaint, once the
/// placeholders drawn before it have recorded where they landed.
struct Indicators(LiveSlots);

impl IntoElement for Indicators {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Indicators {
    type RequestLayoutState = ();
    type PrepaintState = Vec<(AnyElement, Bounds<Pixels>)>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (window.request_layout(Style::default(), None, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let shown: Vec<_> = {
            let registry = (self.0).0.borrow();
            registry
                .slots
                .values()
                .filter(|slot| {
                    let covered =
                        |(_, cover): &(EntityId, Bounds<Pixels>)| cover.intersects(&slot.bounds);
                    !registry.covers.iter().any(covered)
                })
                .map(|slot| (slot.bounds, slot.clip, slot.build.clone()))
                .collect()
        };
        shown
            .into_iter()
            .map(|(bounds, clip, build)| {
                let mut element = build();
                window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
                    element.layout_as_root(bounds.size.into(), window, cx);
                    element.prepaint_at(bounds.origin, window, cx);
                });
                (element, clip)
            })
            .collect()
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        indicators: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        for (element, clip) in indicators {
            window.with_content_mask(Some(ContentMask { bounds: *clip }), |window| {
                element.paint(window, cx)
            });
        }
    }
}
