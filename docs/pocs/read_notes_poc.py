import subprocess
import json

def get_apple_notes():
    # AppleScript: 取得前 5 則備忘錄的標題與內容
    # 使用 JSON 格式輸出以便處理
    script = '''
    tell application "Notes"
        set noteList to {}
        set allNotes to notes
        set noteCount to count of allNotes
        if noteCount > 5 then set noteCount to 5
        
        repeat with i from 1 to noteCount
            set theNote to item i of allNotes
            set noteName to name of theNote
            set noteBody to body of theNote
            set end of noteList to {title:noteName, body:noteBody}
        end repeat
        return noteList
    end tell
    '''
    
    try:
        # 執行 osascript 並取得輸出
        process = subprocess.Popen(['osascript', '-e', script], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        stdout, stderr = process.communicate()
        
        if stderr:
            print(f"錯誤: {stderr}")
            return None
            
        return stdout.strip()
    except Exception as e:
        print(f"執行出錯: {e}")
        return None

if __name__ == "__main__":
    print("正在讀取 Apple 備忘錄...\n")
    notes_data = get_apple_notes()
    if notes_data:
        print("--- 讀取結果 ---")
        print(notes_data)
    else:
        print("未能取得備忘錄資訊。請確保備忘錄 App 中有內容，且已授予權限。")
