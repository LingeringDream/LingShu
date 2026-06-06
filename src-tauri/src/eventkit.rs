// ── EventKit Bridge (Phase B) ──────────────────────────────────────────
//
// Tauri commands that bridge macOS EventKit for reading/writing
// Apple Calendar events. Only compiled on macOS (target_os = "macos").
//
// EKEventStore is not Send+Sync, so we store it in a thread-local
// RefCell. Each command creates or reuses the store lazily.

use serde::{Deserialize, Serialize};

// ── macOS-specific implementation ─────────────────────────────────────

#[cfg(target_os = "macos")]
mod imp {
    use std::cell::RefCell;

    use objc2::rc::Retained;
    use objc2_event_kit::{
        EKAuthorizationStatus, EKCalendar, EKEntityType, EKEvent, EKEventStore, EKSpan,
    };
    use objc2_foundation::{NSDate, NSError, NSString};

    thread_local! {
        static STORE: RefCell<Option<Retained<EKEventStore>>> = const { RefCell::new(None) };
    }

    fn with_store<F, R>(f: F) -> Result<R, String>
    where
        F: FnOnce(&EKEventStore) -> Result<R, String>,
    {
        STORE.with(|cell| {
            let mut opt = cell.borrow_mut();
            if opt.is_none() {
                *opt = Some(unsafe { EKEventStore::new() });
            }
            let store = opt.as_ref().unwrap();
            f(store)
        })
    }

    fn find_calendar(store: &EKEventStore, name: &str) -> Option<Retained<EKCalendar>> {
        let calendars = unsafe { store.calendarsForEntityType(EKEntityType::Event) };
        for cal in calendars.iter() {
            let title = unsafe { cal.title() };
            if title.to_string().to_lowercase() == name.to_lowercase() {
                return Some(cal.clone());
            }
        }
        unsafe { store.defaultCalendarForNewEvents() }
    }

    fn iso_to_nsdate(iso: &str) -> Result<Retained<NSDate>, String> {
        let dt = chrono::DateTime::parse_from_rfc3339(iso)
            .map_err(|e| format!("Invalid ISO 8601 date '{}': {}", iso, e))?;
        let timestamp = dt.naive_utc().and_utc().timestamp();
        let nsdate = NSDate::dateWithTimeIntervalSince1970(objc2_foundation::NSTimeInterval::from(
            timestamp as f64,
        ));
        Ok(nsdate)
    }

    // ── Public API ─────────────────────────────────────────────────

    pub fn request_access() -> Result<String, String> {
        let status = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };

        match status {
            EKAuthorizationStatus::FullAccess => Ok("full_access".into()),
            EKAuthorizationStatus::Denied => Ok("denied".into()),
            EKAuthorizationStatus::Restricted => Ok("restricted".into()),
            EKAuthorizationStatus::WriteOnly => {
                trigger_access_prompt();
                Ok("write_only".into())
            }
            _ => {
                // NotDetermined
                trigger_access_prompt();
                Ok("not_determined".into())
            }
        }
    }

    pub fn create_event(
        title: &str,
        start_time: &str,
        end_time: &str,
        location: Option<&str>,
        notes: Option<&str>,
        calendar_name: Option<&str>,
    ) -> Result<String, String> {
        with_store(|store| {
            let event = unsafe { EKEvent::eventWithEventStore(store) };

            unsafe { event.setTitle(Some(&NSString::from_str(title))) };

            let start = iso_to_nsdate(start_time)?;
            let end = iso_to_nsdate(end_time)?;
            unsafe {
                event.setStartDate(Some(&start));
                event.setEndDate(Some(&end));
            }

            if let Some(loc) = location {
                unsafe { event.setLocation(Some(&NSString::from_str(loc))) };
            }
            if let Some(n) = notes {
                unsafe { event.setNotes(Some(&NSString::from_str(n))) };
            }

            let cal_name = calendar_name.unwrap_or("default");
            if let Some(cal) = find_calendar(store, cal_name) {
                unsafe { event.setCalendar(Some(&cal)) };
            }

            let result =
                unsafe { store.saveEvent_span_commit_error(&event, EKSpan::ThisEvent, true) };

            match result {
                Ok(()) => {
                    let id = unsafe { event.eventIdentifier() }
                        .map(|i| i.to_string())
                        .unwrap_or_default();
                    Ok(id)
                }
                Err(err) => {
                    let msg = err.localizedDescription().to_string();
                    Err(format!("Failed to save event: {}", msg))
                }
            }
        })
    }

    pub fn update_event(
        event_identifier: &str,
        new_title: Option<&str>,
        new_start: Option<&str>,
        new_end: Option<&str>,
        new_location: Option<&str>,
        new_notes: Option<&str>,
    ) -> Result<(), String> {
        with_store(|store| {
            let ns_id = NSString::from_str(event_identifier);
            let existing = unsafe { store.eventWithIdentifier(&ns_id) }
                .ok_or_else(|| format!("Event not found: {}", event_identifier))?;

            if let Some(t) = new_title {
                unsafe { existing.setTitle(Some(&NSString::from_str(t))) };
            }
            if let Some(s) = new_start {
                let d = iso_to_nsdate(s)?;
                unsafe { existing.setStartDate(Some(&d)) };
            }
            if let Some(e) = new_end {
                let d = iso_to_nsdate(e)?;
                unsafe { existing.setEndDate(Some(&d)) };
            }
            if let Some(l) = new_location {
                unsafe { existing.setLocation(Some(&NSString::from_str(l))) };
            }
            if let Some(n) = new_notes {
                unsafe { existing.setNotes(Some(&NSString::from_str(n))) };
            }

            let result =
                unsafe { store.saveEvent_span_commit_error(&existing, EKSpan::ThisEvent, true) };

            match result {
                Ok(()) => Ok(()),
                Err(err) => Err(format!(
                    "Failed to update event: {}",
                    err.localizedDescription()
                )),
            }
        })
    }

    pub fn delete_event(event_identifier: &str) -> Result<(), String> {
        with_store(|store| {
            let ns_id = NSString::from_str(event_identifier);
            let existing = unsafe { store.eventWithIdentifier(&ns_id) }
                .ok_or_else(|| format!("Event not found: {}", event_identifier))?;

            let result =
                unsafe { store.removeEvent_span_commit_error(&existing, EKSpan::ThisEvent, true) };

            match result {
                Ok(()) => Ok(()),
                Err(err) => Err(format!(
                    "Failed to delete event: {}",
                    err.localizedDescription()
                )),
            }
        })
    }

    fn trigger_access_prompt() {
        let _ = with_store(|store| {
            use std::sync::{Arc, Mutex};

            let done = Arc::new(Mutex::new(false));
            let done_clone = done.clone();

            let block = block2::RcBlock::new(
                move |_granted: objc2::runtime::Bool, _error: *mut NSError| {
                    let mut d = done_clone.lock().unwrap();
                    *d = true;
                },
            );

            let ptr: *mut block2::DynBlock<_> =
                &*block as *const block2::DynBlock<_> as *mut block2::DynBlock<_>;
            unsafe {
                store.requestFullAccessToEventsWithCompletion(ptr);
            }

            // Pump for up to 10 seconds for the user to respond.
            let start = std::time::Instant::now();
            while !*done.lock().unwrap() && start.elapsed() < std::time::Duration::from_secs(10) {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            Ok(())
        });
    }
}

// ── Command argument types (shared between platforms) ─────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEventInput {
    pub title: String,
    pub start_time: String,
    pub end_time: String,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub calendar_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEventArgs {
    pub event_identifier: String,
    pub title: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
}

// ── Tauri Commands ────────────────────────────────────────────────────

#[tauri::command]
pub fn request_calendar_access() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        imp::request_access()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok("unsupported".into())
    }
}

#[tauri::command]
pub fn create_calendar_event(event: CalendarEventInput) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        imp::create_event(
            &event.title,
            &event.start_time,
            &event.end_time,
            event.location.as_deref(),
            event.notes.as_deref(),
            event.calendar_name.as_deref(),
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = event;
        Err("EventKit is only available on macOS".into())
    }
}

#[tauri::command]
pub fn update_calendar_event(updates: UpdateEventArgs) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        imp::update_event(
            &updates.event_identifier,
            updates.title.as_deref(),
            updates.start_time.as_deref(),
            updates.end_time.as_deref(),
            updates.location.as_deref(),
            updates.notes.as_deref(),
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = updates;
        Err("EventKit is only available on macOS".into())
    }
}

#[tauri::command]
pub fn delete_calendar_event(event_identifier: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        imp::delete_event(&event_identifier)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = event_identifier;
        Err("EventKit is only available on macOS".into())
    }
}
