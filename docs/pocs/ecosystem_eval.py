import subprocess

def run_script(script):
    process = subprocess.Popen(['osascript', '-e', script], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    stdout, stderr = process.communicate()
    return stdout.strip()

def evaluate_ecosystem():
    print("🚀 正在啟動全方位生產力評估...")
    
    # 1. 取得備忘錄數量
    notes_count = run_script('tell application "Notes" to get count of notes')
    print(f"📝 備忘錄: 共有 {notes_count} 則筆記")
    
    # 2. 取得今日待辦清單 (Reminders)
    reminders_script = '''
    tell application "Reminders"
        set incompleteTasks to (reminders whose completed is false)
        set taskList to {}
        repeat with t in incompleteTasks
            set end of taskList to (name of t)
        end repeat
        return taskList
    end tell
    '''
    tasks = run_script(reminders_script)
    print(f"✅ 待辦清單: {tasks if tasks else '今日無未完成任務'}")
    
    # 3. 取得今日行事曆行程 (Calendar)
    calendar_script = '''
    tell application "Calendar"
        set now to current date
        set endOfDay to now + (24 * 60 * 60)
        set eventList to {}
        repeat with i from 1 to (count of calendars)
            set theCal to calendar i
            set theEvents to (every event of theCal whose start date is greater than or equal to now and start date is less than or equal to endOfDay)
            repeat with e in theEvents
                set end of eventList to (summary of e & " (" & (start date of e as string) & ")")
            end repeat
        end repeat
        return eventList
    end tell
    '''
    events = run_script(calendar_script)
    print(f"📅 行事曆: {events if events else '今日無行程'}")

if __name__ == "__main__":
    evaluate_ecosystem()
