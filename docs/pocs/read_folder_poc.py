import subprocess

def get_notes_from_folder(folder_name):
    script = f'''
    tell application "Notes"
        try
            set theNotes to notes of folder "{folder_name}"
            set noteList to {{}}
            repeat with theNote in theNotes
                set end of noteList to (name of theNote)
            end repeat
            return noteList
        on error
            return "錯誤: 找不到資料夾 '{folder_name}'"
        end try
    end tell
    '''
    
    process = subprocess.Popen(['osascript', '-e', script], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    stdout, stderr = process.communicate()
    return stdout.strip()

if __name__ == "__main__":
    target = "threads"  # 你可以改成 "threads" 或 "收藏"
    print(f"正在讀取資料夾 '{target}' 中的標題...\n")
    result = get_notes_from_folder(target)
    print(result)
