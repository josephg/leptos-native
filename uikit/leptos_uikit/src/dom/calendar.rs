//! Safe EventKit wrapper — list calendars, add/remove events, and
//! trigger the system calendar-permission dialog. Feature-gated
//! (`calendar`) so apps that don't touch the calendar don't pull in
//! `objc2-event-kit`.
//!
//! Requires `NSCalendarsFullAccessUsageDescription` (and, pre-iOS 17,
//! `NSCalendarsUsageDescription`) in the app's Info.plist — without
//! it the process is killed on the first access request.
//!
//! Mirrors `storage.rs`'s role: all objc2 stays inside this module so
//! app crates get a plain-Rust API. Completion callbacks are hopped
//! back to the main thread before invoking user closures.

use std::cell::RefCell;
use std::sync::Mutex;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_event_kit::{EKEntityType, EKEvent, EKEventStore, EKSpan};
use objc2_foundation::{NSDate, NSError, NSString, NSURL};

/// One of the user's calendars, as shown in a picker.
#[derive(Clone, Debug)]
pub struct CalendarInfo {
    pub id: String,
    pub title: String,
    pub writable: bool,
}

/// The fields of a calendar event to create.
#[derive(Clone, Debug, Default)]
pub struct EventSpec {
    pub title: String,
    pub start_epoch: f64,
    pub end_epoch: f64,
    pub location: String,
    pub notes: String,
    pub url: String,
}

/// Calendar-database failure (save/remove rejected, unknown ids, …).
#[derive(Clone, Debug)]
pub struct CalendarError(pub String);

impl std::fmt::Display for CalendarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "calendar error: {}", self.0)
    }
}

impl std::error::Error for CalendarError {}

thread_local! {
    static STORE: RefCell<Option<Retained<EKEventStore>>> =
        const { RefCell::new(None) };
}

fn with_store<R>(f: impl FnOnce(&EKEventStore) -> R) -> R {
    STORE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let store = slot.get_or_insert_with(|| unsafe { EKEventStore::new() });
        f(store)
    })
}

/// Ask the user for full calendar access (iOS 17+ dialog). `cb` is
/// invoked on the main thread with `true` when access was granted.
/// If access was already granted or denied, the callback fires with
/// the existing verdict without showing a dialog again. `Send`
/// because EventKit delivers the completion off the main thread; the
/// wrapper hops back before calling `cb`.
pub fn request_access(cb: impl FnOnce(bool) + Send + 'static) {
    let cb: Mutex<Option<Box<dyn FnOnce(bool) + Send + 'static>>> =
        Mutex::new(Some(Box::new(cb)));

    let block = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
        let Ok(mut guard) = cb.lock() else { return };
        let Some(f) = guard.take() else { return };
        let granted = granted.as_bool();
        leptos_apple_shared::on_main(move || {
            f(granted);
        });
    });

    with_store(|store| unsafe {
        store.requestFullAccessToEventsWithCompletion(
            &*block as *const _ as *mut _,
        );
    });
}

/// The user's event calendars, writable ones first.
pub fn calendars() -> Vec<CalendarInfo> {
    with_store(|store| {
        let list = unsafe { store.calendarsForEntityType(EKEntityType::Event) };
        let mut out: Vec<CalendarInfo> = list
            .iter()
            .map(|c| unsafe {
                CalendarInfo {
                    id: c.calendarIdentifier().to_string(),
                    title: c.title().to_string(),
                    writable: c.allowsContentModifications(),
                }
            })
            .collect();
        out.sort_by_key(|c| !c.writable);
        out
    })
}

/// Create an event in the calendar with id `calendar_id`. Returns the
/// new event's identifier for later [`remove_event`].
pub fn add_event(
    calendar_id: &str,
    spec: &EventSpec,
) -> Result<String, CalendarError> {
    with_store(|store| unsafe {
        let calendar = store
            .calendarWithIdentifier(&NSString::from_str(calendar_id))
            .ok_or_else(|| {
                CalendarError(format!("unknown calendar {calendar_id}"))
            })?;

        let event = EKEvent::eventWithEventStore(store);
        event.setCalendar(Some(&calendar));
        event.setTitle(Some(&NSString::from_str(&spec.title)));
        event.setStartDate(Some(&NSDate::dateWithTimeIntervalSince1970(
            spec.start_epoch,
        )));
        event.setEndDate(Some(&NSDate::dateWithTimeIntervalSince1970(
            spec.end_epoch,
        )));
        if !spec.location.is_empty() {
            event.setLocation(Some(&NSString::from_str(&spec.location)));
        }
        if !spec.notes.is_empty() {
            event.setNotes(Some(&NSString::from_str(&spec.notes)));
        }
        if !spec.url.is_empty() {
            if let Some(u) = NSURL::URLWithString(&NSString::from_str(&spec.url))
            {
                event.setURL(Some(&u));
            }
        }

        store
            .saveEvent_span_error(&event, EKSpan::ThisEvent)
            .map_err(|e| CalendarError(e.localizedDescription().to_string()))?;

        event
            .eventIdentifier()
            .map(|s| s.to_string())
            .ok_or_else(|| CalendarError("saved event has no id".into()))
    })
}

/// Remove a previously added event. Removing an event that's already
/// gone is not an error.
pub fn remove_event(event_identifier: &str) -> Result<(), CalendarError> {
    with_store(|store| unsafe {
        let Some(event) =
            store.eventWithIdentifier(&NSString::from_str(event_identifier))
        else {
            return Ok(());
        };
        store
            .removeEvent_span_error(&event, EKSpan::ThisEvent)
            .map_err(|e| CalendarError(e.localizedDescription().to_string()))
    })
}
