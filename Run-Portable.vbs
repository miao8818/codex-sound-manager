Option Explicit
Dim shell, fileSystem, root, executable
Set shell = CreateObject("WScript.Shell")
Set fileSystem = CreateObject("Scripting.FileSystemObject")
root = fileSystem.GetParentFolderName(WScript.ScriptFullName)
executable = fileSystem.BuildPath(root, "codex-sound-manager.exe")
If Not fileSystem.FileExists(executable) Then
  executable = fileSystem.BuildPath(root, "target\release\codex-sound-manager.exe")
End If
If Not fileSystem.FileExists(executable) Then
  MsgBox "Release executable not found. Run Build-Release.cmd first.", 48, "Codex Sound Manager"
  WScript.Quit 1
End If
shell.Run Chr(34) & executable & Chr(34), 0, False
