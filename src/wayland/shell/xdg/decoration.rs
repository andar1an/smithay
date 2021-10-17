//! XDG Window decoration manager
//!
//! This interface allows a compositor to announce support for server-side decorations.
//!
//! A client can use this protocol to request being decorated by a supporting compositor.
//!
//!
//! ```no_run
//! # extern crate wayland_server;
//! #
//! use smithay::wayland::shell::xdg::decoration::{init_xdg_decoration_manager, XdgDecorationRequest};
//! use smithay::reexports::wayland_protocols::unstable::xdg_decoration::v1::server::zxdg_toplevel_decoration_v1::Mode;
//!
//! # let mut display = wayland_server::Display::new();
//!
//! init_xdg_decoration_manager(
//!     &mut display,
//!     |req, _ddata| match req {
//!         XdgDecorationRequest::NewToplevelDecoration { toplevel } => {
//!             let res = toplevel.with_pending_state(|state| {
//!                   // Advertise server side decoration
//!                 state.decoration_mode = Some(Mode::ServerSide);
//!             });
//!
//!             if res.is_ok() {
//!                 toplevel.send_configure();
//!             }
//!         }
//!         XdgDecorationRequest::SetMode { .. } => {}
//!         XdgDecorationRequest::UnsetMode { .. } => {}
//!     },
//!     None,
//! );
//!

use std::{cell::RefCell, ops::Deref, rc::Rc};
use wayland_protocols::unstable::xdg_decoration::v1::server::{
    zxdg_decoration_manager_v1::{self, ZxdgDecorationManagerV1},
    zxdg_toplevel_decoration_v1::{self, Mode, ZxdgToplevelDecorationV1},
};
use wayland_server::{DispatchData, Display, Filter, Global, Main};

use super::ToplevelSurface;
use crate::wayland::shell::xdg::xdg_handlers::ShellSurfaceUserData;

/// Events generated by xdg decoration manager
#[derive(Debug)]
pub enum XdgDecorationRequest {
    /// A new toplevel decoration was instantiated
    NewToplevelDecoration {
        /// The toplevel asosiated with decoration
        toplevel: ToplevelSurface,
    },
    /// Informs the compositor that the client prefers the provided decoration mode.
    SetMode {
        /// The toplevel asosiated with decoration
        toplevel: ToplevelSurface,
        /// The decoration mode
        mode: Mode,
    },
    /// Informs the compositor that the client doesn't prefer a particular decoration mode.
    UnsetMode {
        /// The toplevel asosiated with decoration
        toplevel: ToplevelSurface,
    },
}

/// Create a new XDG Decoration Manager global
pub fn init_xdg_decoration_manager<L, Impl>(
    display: &mut Display,
    implementation: Impl,
    _logger: L,
) -> Global<ZxdgDecorationManagerV1>
where
    L: Into<Option<::slog::Logger>>,
    Impl: FnMut(XdgDecorationRequest, DispatchData<'_>) + 'static,
{
    let cb = Rc::new(RefCell::new(implementation));
    display.create_global(
        1,
        Filter::new(
            move |(manager, _version): (Main<ZxdgDecorationManagerV1>, _), _, _| {
                let cb = cb.clone();
                manager.quick_assign(move |_manager, request, ddata| {
                    match request {
                        zxdg_decoration_manager_v1::Request::Destroy => {
                            // All is handled by destructor.
                        }
                        zxdg_decoration_manager_v1::Request::GetToplevelDecoration { id, toplevel } => {
                            if let Some(data) = toplevel.as_ref().user_data().get::<ShellSurfaceUserData>() {
                                if data.decoration.borrow().is_none() {
                                    *data.decoration.borrow_mut() = Some(id.deref().clone());
                                } else {
                                    use wayland_protocols::unstable::xdg_decoration::v1::server::zxdg_toplevel_decoration_v1::Error; 
                                    id.as_ref().post_error(Error::AlreadyConstructed as u32, "toplevel decoration is already constructed".to_string());
                                }

                                let toplevel = ToplevelSurface {
                                    shell_surface: toplevel.clone(),
                                    wl_surface: data.wl_surface.clone(),
                                };

                                (&mut *cb.borrow_mut())(
                                    XdgDecorationRequest::NewToplevelDecoration {
                                        toplevel: toplevel.clone(),
                                    },
                                    ddata,
                                );

                                let cb = cb.clone();
                                id.quick_assign(move |_, request, ddata| match request {
                                    zxdg_toplevel_decoration_v1::Request::SetMode { mode } => {
                                        (&mut *cb.borrow_mut())(
                                            XdgDecorationRequest::SetMode {
                                                toplevel: toplevel.clone(),
                                                mode,
                                            },
                                            ddata,
                                        );
                                    }
                                    zxdg_toplevel_decoration_v1::Request::UnsetMode => {
                                        (&mut *cb.borrow_mut())(
                                            XdgDecorationRequest::UnsetMode {
                                                toplevel: toplevel.clone(),
                                            },
                                            ddata,
                                        );
                                    }
                                    _ => {}
                                });
                            }

                            id.assign_destructor(Filter::new(
                                move |_decoration: ZxdgToplevelDecorationV1, _, _| {
                                    if let Some(data) =
                                        toplevel.as_ref().user_data().get::<ShellSurfaceUserData>()
                                    {
                                        *data.decoration.borrow_mut() = None;
                                    }
                                },
                            ));
                        }
                        _ => unreachable!(),
                    }
                });
            },
        ),
    )
}

pub(super) fn send_decoration_configure(id: &ZxdgToplevelDecorationV1, mode: Mode) {
    id.configure(mode)
}