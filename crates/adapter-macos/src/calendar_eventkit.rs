use chrono::{DateTime, Local, TimeZone};
use objc2::rc::Retained;
use objc2_event_kit::{EKAuthorizationStatus, EKCalendar, EKEntityType, EKEvent, EKEventStore};
use objc2_foundation::NSArray;

use crate::MacosError;

use super::CalendarEvent;

pub(super) fn list_events(
    from: DateTime<Local>,
    to: DateTime<Local>,
    calendar_filter: Option<&str>,
) -> Result<Option<Vec<CalendarEvent>>, MacosError> {
    if !authorization_allows_event_reads() {
        return Ok(None);
    }

    let store = unsafe { EKEventStore::new() };
    let calendars = filtered_calendars(&store, calendar_filter)?;
    let from_date =
        objc2_foundation::NSDate::dateWithTimeIntervalSince1970(from.timestamp() as f64);
    let to_date = objc2_foundation::NSDate::dateWithTimeIntervalSince1970(to.timestamp() as f64);
    let predicate = unsafe {
        store.predicateForEventsWithStartDate_endDate_calendars(
            &from_date,
            &to_date,
            calendars.as_deref(),
        )
    };
    let events = unsafe { store.eventsMatchingPredicate(&predicate) };
    let items = events
        .to_vec()
        .into_iter()
        .map(|event| event_to_item(&event))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(items))
}

fn authorization_allows_event_reads() -> bool {
    let status = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
    status == EKAuthorizationStatus::FullAccess
}

fn filtered_calendars(
    store: &EKEventStore,
    calendar_filter: Option<&str>,
) -> Result<Option<Retained<NSArray<EKCalendar>>>, MacosError> {
    let Some(filter) = calendar_filter else {
        return Ok(None);
    };

    let calendars = unsafe { store.calendarsForEntityType(EKEntityType::Event) };
    let matching = calendars
        .to_vec()
        .into_iter()
        .filter(|calendar| unsafe { calendar.title() }.to_string() == filter)
        .collect::<Vec<_>>();

    Ok(Some(NSArray::from_retained_slice(&matching)))
}

fn event_to_item(event: &EKEvent) -> Result<CalendarEvent, MacosError> {
    let title = unsafe { event.title() }.to_string();
    let start = nsdate_to_calendar_string(unsafe { event.startDate() }.as_ref())?;
    let end = nsdate_to_calendar_string(unsafe { event.endDate() }.as_ref())?;
    let calendar = unsafe { event.calendar() }
        .map(|calendar| unsafe { calendar.title() }.to_string())
        .unwrap_or_default();
    let location = unsafe { event.location() }
        .map(|value| value.to_string())
        .unwrap_or_default();
    let notes = unsafe { event.notes() }
        .map(|value| value.to_string())
        .unwrap_or_default();
    let all_day = unsafe { event.isAllDay() };

    Ok(CalendarEvent {
        title,
        start,
        end,
        calendar,
        location,
        notes,
        all_day,
    })
}

fn nsdate_to_calendar_string(date: &objc2_foundation::NSDate) -> Result<String, MacosError> {
    let timestamp = date.timeIntervalSince1970();
    let local = Local
        .timestamp_opt(timestamp as i64, 0)
        .single()
        .ok_or_else(|| MacosError::Other(format!("invalid EventKit date: {timestamp}")))?;
    Ok(local.format("%Y-%m-%dT%H:%M:%S").to_string())
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};

    use super::nsdate_to_calendar_string;

    #[test]
    fn eventkit_dates_format_like_calendar_output() {
        let local = Local
            .with_ymd_and_hms(2026, 4, 21, 16, 30, 45)
            .single()
            .expect("local datetime");
        let date =
            objc2_foundation::NSDate::dateWithTimeIntervalSince1970(local.timestamp() as f64);

        let formatted = nsdate_to_calendar_string(&date).expect("formatted date");

        assert_eq!(formatted, "2026-04-21T16:30:45");
    }
}
