import subprocess

def move_note(note_title, from_folder, to_folder):
    # AppleScript: 尋找筆記並移動到指定資料夾
    script = f'''
    tell application "Notes"
        try
            set theNote to (first note of folder "{from_folder}" whose name is "{note_title}")
            set destFolder to folder "{to_folder}"
            move theNote to destFolder
            return "成功: 已將 '{note_title}' 從 '{from_folder}' 移動到 '{to_folder}'"
        on error errStr
            return "錯誤: " & errStr
        end try
    end tell
    '''
    
    process = subprocess.Popen(['osascript', '-e', script], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    stdout, stderr = process.communicate()
    return stdout.strip()

if __name__ == "__main__":
    # 範例：將 "Harness engineering for coding agent users" 從 "Notes" 移動到 "threads" (或反向測試)
    # 我們剛才看到它已經在 "threads" 了，所以我們試著把它搬到 "idea" 試試看
    note_name = "Harness engineering for coding agent users"
    source = "threads"
    destination = "idea"
    
    print(f"正在試圖移動筆記 '{note_name}'...\n")
    result = move_note(note_name, source, destination)
    print(result)
