use std::sync::mpsc;
use std::time::Duration as StdDuration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_event_kit::{
    EKAuthorizationStatus, EKCalendar, EKEntityType, EKEventStore, EKReminder,
};
use objc2_foundation::{NSArray, NSDateComponentUndefined};

use crate::MacosError;

use super::ReminderItem;

pub(super) fn list(list_filter: Option<&str>) -> Result<Option<Vec<ReminderItem>>, MacosError> {
    if !authorization_allows_reminder_reads() {
        return Ok(None);
    }

    let store = unsafe { EKEventStore::new() };
    let calendars = filtered_calendars(&store, list_filter)?;
    let predicate = unsafe { store.predicateForRemindersInCalendars(calendars.as_deref()) };
    let reminders = fetch_reminders(&store, &predicate)?;
    let items = reminders
        .iter()
        .map(|reminder| reminder_to_item(reminder))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(items))
}

fn authorization_allows_reminder_reads() -> bool {
    let status = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Reminder) };
    status == EKAuthorizationStatus::FullAccess
}

fn filtered_calendars(
    store: &EKEventStore,
    list_filter: Option<&str>,
) -> Result<Option<Retained<NSArray<EKCalendar>>>, MacosError> {
    let Some(filter) = list_filter else {
        return Ok(None);
    };

    let calendars = unsafe { store.calendarsForEntityType(EKEntityType::Reminder) };
    let matching = calendars
        .to_vec()
        .into_iter()
        .filter(|calendar| unsafe { calendar.title() }.to_string() == filter)
        .collect::<Vec<_>>();

    Ok(Some(NSArray::from_retained_slice(&matching)))
}

fn fetch_reminders(
    store: &EKEventStore,
    predicate: &objc2_foundation::NSPredicate,
) -> Result<Vec<Retained<EKReminder>>, MacosError> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let completion = RcBlock::new(move |reminders_ptr: *mut NSArray<EKReminder>| {
        let reminders = unsafe { reminders_ptr.as_ref() }
            .map(|array| array.to_vec())
            .unwrap_or_default();
        let _ = sender.send(reminders);
    });

    let _fetch_id = unsafe {
        store.fetchRemindersMatchingPredicate_completion(predicate, &completion)
    };

    receiver
        .recv_timeout(StdDuration::from_secs(10))
        .map_err(|_| MacosError::Other("eventkit reminders fetch timed out".into()))
}

fn reminder_to_item(reminder: &EKReminder) -> Result<ReminderItem, MacosError> {
    let id = unsafe { reminder.calendarItemIdentifier() }.to_string();
    let title = unsafe { reminder.title() }.to_string();
    let notes = unsafe { reminder.notes() }
        .map(|notes| notes.to_string())
        .unwrap_or_default();
    let due_date = unsafe { reminder.dueDateComponents() }
        .as_deref()
        .and_then(date_components_to_string);
    let completed = unsafe { reminder.isCompleted() };
    let list_name = unsafe { reminder.calendar() }
        .map(|calendar| unsafe { calendar.title() }.to_string())
        .unwrap_or_default();

    Ok(ReminderItem {
        id: format!("x-apple-reminder://{id}"),
        title,
        notes,
        due_date,
        completed,
        list_name,
    })
}

fn date_components_to_string(components: &objc2_foundation::NSDateComponents) -> Option<String> {
    let year = components.year();
    let month = components.month();
    let day = components.day();

    if year == NSDateComponentUndefined
        || month == NSDateComponentUndefined
        || day == NSDateComponentUndefined
    {
        return None;
    }

    let hour = zero_if_undefined(components.hour());
    let minute = zero_if_undefined(components.minute());
    let second = zero_if_undefined(components.second());

    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}"
    ))
}

fn zero_if_undefined(value: isize) -> isize {
    if value == NSDateComponentUndefined {
        0
    } else {
        value
    }
}
