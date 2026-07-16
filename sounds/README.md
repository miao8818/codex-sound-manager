# Sounds

`default-notification.wav` 是本项目原创生成的默认任务完成提示音，随 MIT 许可分发。

可通过以下命令重新生成：

```powershell
python .\scripts\generate_default_sound.py
```

用户在应用中选择的自定义音频不会写入项目目录，而会复制到：

```text
~\.codex\codex-sound-manager\sounds\
```
