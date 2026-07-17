import { useCallback, useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import {
  AlertTriangle,
  BellRing,
  CheckCircle2,
  CircleDot,
  FileAudio,
  FolderOpen,
  LogOut,
  MessageCircle,
  Minus,
  Minimize2,
  Play,
  Plus,
  RefreshCw,
  RotateCcw,
  Save,
  ShieldCheck,
  Volume2,
  X,
  XCircle,
} from 'lucide-react'
import packageInfo from '../package.json'
import communityQr from '../docs/images/community-qr.jpg'
import { Button } from './components/ui/button'
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogTitle, DialogTrigger } from './components/ui/dialog'
import { Switch } from './components/ui/switch'
import { Tooltip } from './components/ui/tooltip'
import { cn } from './lib/utils'

type AppSettings = {
  enabled: boolean
  playCount: number
  floatingBallEnabled: boolean
  soundPath: string | null
  soundName: string | null
  previousNotifier: string[] | null
  codexHome: string | null
}

type ScanResult = {
  codexFound: boolean
  configFound: boolean
  configured: boolean
  codexHome: string
  configPath: string
  executablePath: string
  soundName: string
  usingDefaultSound: boolean
  settings: AppSettings
  statusMessage: string
}

type OperationResult = {
  message: string
  scan: ScanResult
}

type SoundSelection = {
  path: string
  name: string
}

type Notice = {
  tone: 'success' | 'warning' | 'error'
  message: string
}

const emptySettings: AppSettings = {
  enabled: true,
  playCount: 2,
  floatingBallEnabled: false,
  soundPath: null,
  soundName: null,
  previousNotifier: null,
  codexHome: null,
}

function errorMessage(error: unknown) {
  return typeof error === 'string' ? error : error instanceof Error ? error.message : '发生未知错误'
}

function displayCodexPath(path: string | null | undefined) {
  return path?.replace(/^[A-Z]:\\Users\\[^\\]+(?=\\\.codex(?:\\|$))/i, '~')
}

export default function App() {
  const [scan, setScan] = useState<ScanResult | null>(null)
  const [settings, setSettings] = useState<AppSettings>(emptySettings)
  const [soundName, setSoundName] = useState('内置默认提示音')
  const [busy, setBusy] = useState<'scan' | 'preview' | 'apply' | 'remove' | 'sound' | 'enabled' | 'floating' | 'close' | null>('scan')
  const [notice, setNotice] = useState<Notice | null>(null)
  const [closeDialogOpen, setCloseDialogOpen] = useState(false)

  const syncScan = useCallback((result: ScanResult) => {
    setScan(result)
    setSettings(result.settings)
    setSoundName(result.soundName)
  }, [])

  const refresh = useCallback(async () => {
    setBusy('scan')
    setNotice(null)
    try {
      syncScan(await invoke<ScanResult>('scan_codex'))
    } catch (error) {
      setNotice({ tone: 'error', message: errorMessage(error) })
    } finally {
      setBusy(null)
    }
  }, [syncScan])

  useEffect(() => {
    void refresh()
  }, [refresh])

  useEffect(() => {
    let disposed = false
    const unlisteners: Array<() => void> = []
    const subscribe = <T,>(eventName: string, handler: (payload: T) => void) => {
      void listen<T>(eventName, (event) => {
        if (!disposed) handler(event.payload)
      }).then((unlisten) => {
        if (disposed) {
          unlisten()
        } else {
          unlisteners.push(unlisten)
        }
      }).catch((error) => {
        if (!disposed) {
          setNotice({ tone: 'error', message: `监听程序状态失败：${errorMessage(error)}` })
        }
      })
    }

    subscribe<void>('close-choice-requested', () => setCloseDialogOpen(true))
    subscribe<boolean>('sound-enabled-changed', (enabled) => {
      setSettings((current) => ({ ...current, enabled }))
    })
    subscribe<boolean>('floating-ball-enabled-changed', (floatingBallEnabled) => {
      setSettings((current) => ({ ...current, floatingBallEnabled }))
    })

    return () => {
      disposed = true
      unlisteners.forEach((unlisten) => unlisten())
    }
  }, [])

  const updateCount = (delta: number) => {
    setSettings((current) => ({
      ...current,
      playCount: Math.min(10, Math.max(1, current.playCount + delta)),
    }))
  }

  const chooseSound = async () => {
    setBusy('sound')
    setNotice(null)
    try {
      const selected = await invoke<SoundSelection | null>('choose_sound')
      if (selected) {
        setSettings((current) => ({ ...current, soundPath: selected.path, soundName: selected.name }))
        setSoundName(selected.name)
      }
    } catch (error) {
      setNotice({ tone: 'error', message: errorMessage(error) })
    } finally {
      setBusy(null)
    }
  }

  const resetSound = () => {
    setSettings((current) => ({ ...current, soundPath: null, soundName: null }))
    setSoundName('内置默认提示音')
  }

  const updateSoundEnabled = async (enabled: boolean) => {
    const previous = settings.enabled
    setSettings((current) => ({ ...current, enabled }))
    setBusy('enabled')
    setNotice(null)
    try {
      const soundEnabled = await invoke<boolean>('set_sound_enabled', { enabled })
      setSettings((current) => ({ ...current, enabled: soundEnabled }))
      setNotice({
        tone: 'success',
        message: soundEnabled
          ? '提示音已开启，下一次任务完成立即生效'
          : '提示音已关闭，下一次任务完成不会播放',
      })
    } catch (error) {
      setSettings((current) => ({ ...current, enabled: previous }))
      setNotice({ tone: 'error', message: errorMessage(error) })
    } finally {
      setBusy(null)
    }
  }

  const updateFloatingBall = async (enabled: boolean) => {
    setBusy('floating')
    setNotice(null)
    try {
      const floatingBallEnabled = await invoke<boolean>('set_floating_ball_enabled', { enabled })
      setSettings((current) => ({ ...current, floatingBallEnabled }))
      setNotice({
        tone: 'success',
        message: floatingBallEnabled ? '桌面悬浮球已显示' : '桌面悬浮球已隐藏',
      })
    } catch (error) {
      setNotice({ tone: 'error', message: errorMessage(error) })
    } finally {
      setBusy(null)
    }
  }

  const resolveCloseChoice = async (choice: 'exit' | 'tray' | 'cancel') => {
    setBusy('close')
    try {
      await invoke('resolve_close_choice', { choice })
      setCloseDialogOpen(false)
    } catch (error) {
      setNotice({ tone: 'error', message: errorMessage(error) })
    } finally {
      setBusy(null)
    }
  }

  const preview = async () => {
    setBusy('preview')
    setNotice(null)
    try {
      await invoke('preview_sound', { settings })
      setNotice({ tone: 'success', message: `试听完成：已播放 ${settings.playCount} 次` })
    } catch (error) {
      setNotice({ tone: 'error', message: errorMessage(error) })
    } finally {
      setBusy(null)
    }
  }

  const apply = async () => {
    setBusy('apply')
    setNotice(null)
    try {
      const result = await invoke<OperationResult>('apply_configuration', { settings })
      syncScan(result.scan)
      setNotice({ tone: 'warning', message: result.message })
    } catch (error) {
      setNotice({ tone: 'error', message: errorMessage(error) })
    } finally {
      setBusy(null)
    }
  }

  const remove = async () => {
    setBusy('remove')
    setNotice(null)
    try {
      const result = await invoke<OperationResult>('remove_configuration')
      syncScan(result.scan)
      setNotice({ tone: 'warning', message: result.message })
    } catch (error) {
      setNotice({ tone: 'error', message: errorMessage(error) })
    } finally {
      setBusy(null)
    }
  }

  const status = useMemo(() => {
    if (!scan?.codexFound) {
      return { icon: XCircle, label: '未发现 Codex', className: 'border-red-200 bg-red-50 text-red-700' }
    }
    if (scan.configured) {
      return { icon: CheckCircle2, label: '全局配置已启用', className: 'border-emerald-200 bg-emerald-50 text-emerald-700' }
    }
    return { icon: AlertTriangle, label: '等待应用配置', className: 'border-amber-200 bg-amber-50 text-amber-700' }
  }, [scan])

  const StatusIcon = status.icon
  const disabled = busy !== null

  return (
    <div className="min-h-screen bg-background text-foreground">
      <header className="border-b border-border bg-white">
        <div className="mx-auto flex h-16 max-w-[920px] items-center justify-between px-7">
          <div className="flex min-w-0 items-center gap-3.5">
            <div className="grid size-10 shrink-0 place-items-center rounded-lg bg-foreground text-white shadow-sm">
              <BellRing className="size-5" aria-hidden="true" />
            </div>
            <div className="min-w-0">
              <h1 className="truncate text-[18px] font-bold leading-6">Codex 提示音管理器</h1>
              <p className="truncate text-xs text-muted-foreground">Windows 全局任务完成通知</p>
            </div>
          </div>
          <div className={cn('flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-semibold', status.className)}>
            <StatusIcon className="size-3.5" aria-hidden="true" />
            {status.label}
          </div>
        </div>
      </header>

      <main className="mx-auto max-w-[920px] px-7 py-5">
        <section className="mb-3 flex min-h-[68px] items-center justify-between gap-5 border border-border bg-white px-5 shadow-panel">
          <div className="flex min-w-0 items-center gap-3.5">
            <div className={cn('grid size-9 shrink-0 place-items-center rounded-md', scan?.codexFound ? 'bg-emerald-50 text-emerald-700' : 'bg-red-50 text-red-700')}>
              <ShieldCheck className="size-[18px]" aria-hidden="true" />
            </div>
            <div className="min-w-0">
              <p className="text-sm font-semibold">{scan?.statusMessage ?? '正在扫描 Codex'}</p>
              <p className="mt-1 truncate font-mono text-[11px] text-muted-foreground" title={scan?.configPath}>
                {displayCodexPath(scan?.configPath) ?? '正在读取全局配置路径...'}
              </p>
            </div>
          </div>
          <Tooltip label="重新扫描 Codex">
            <Button variant="ghost" size="icon" onClick={() => void refresh()} disabled={disabled} aria-label="重新扫描 Codex">
              <RefreshCw className={cn('size-4', busy === 'scan' && 'animate-spin')} />
            </Button>
          </Tooltip>
        </section>

        <section className="overflow-hidden border border-border bg-white shadow-panel">
          <div className="grid min-h-[82px] grid-cols-2 border-b border-border px-6 py-4">
            <div className="flex items-center justify-between gap-5 border-r border-border pr-6">
              <div>
                <div className="flex items-center gap-2.5">
                  <Volume2 className="size-[18px] text-primary" aria-hidden="true" />
                  <h2 className="text-sm font-bold">任务完成提示音</h2>
                </div>
                <p className="mt-1.5 text-xs text-muted-foreground">{settings.enabled ? '当前已开启' : '当前已关闭'}</p>
              </div>
              <Switch
                checked={settings.enabled}
                onCheckedChange={(enabled) => void updateSoundEnabled(enabled)}
                disabled={disabled}
                aria-label="启用任务完成提示音"
              />
            </div>
            <div className="flex items-center justify-between gap-5 pl-6">
              <div>
                <div className="flex items-center gap-2.5">
                  <CircleDot className="size-[18px] text-[#2563eb]" aria-hidden="true" />
                  <h2 className="text-sm font-bold">桌面悬浮球</h2>
                </div>
                <p className="mt-1.5 text-xs text-muted-foreground">{settings.floatingBallEnabled ? '已显示，双击球体切换提示音' : '当前已隐藏'}</p>
              </div>
              <Switch
                checked={settings.floatingBallEnabled}
                onCheckedChange={(enabled) => void updateFloatingBall(enabled)}
                disabled={disabled}
                aria-label="启用桌面悬浮球"
              />
            </div>
          </div>

          <div className="grid min-h-[90px] grid-cols-[1fr_auto] items-center gap-8 border-b border-border px-6 py-4">
            <div>
              <h2 className="text-sm font-bold">播放次数</h2>
              <p className="mt-1.5 text-xs text-muted-foreground">每次任务完成连续播放</p>
            </div>
            <div className="flex h-10 items-center overflow-hidden rounded-md border border-border bg-white">
              <Tooltip label="减少一次">
                <Button
                  variant="ghost"
                  size="icon"
                  className="rounded-none border-r border-border"
                  onClick={() => updateCount(-1)}
                  disabled={disabled || settings.playCount <= 1}
                  aria-label="减少播放次数"
                >
                  <Minus className="size-4" />
                </Button>
              </Tooltip>
              <output className="w-16 text-center text-sm font-bold tabular-nums" aria-label={`播放 ${settings.playCount} 次`}>
                {settings.playCount} 次
              </output>
              <Tooltip label="增加一次">
                <Button
                  variant="ghost"
                  size="icon"
                  className="rounded-none border-l border-border"
                  onClick={() => updateCount(1)}
                  disabled={disabled || settings.playCount >= 10}
                  aria-label="增加播放次数"
                >
                  <Plus className="size-4" />
                </Button>
              </Tooltip>
            </div>
          </div>

          <div className="grid min-h-[108px] grid-cols-[1fr_auto] items-center gap-6 px-6 py-4">
            <div className="min-w-0">
              <div className="flex items-center gap-2.5">
                <FileAudio className="size-[18px] text-primary" aria-hidden="true" />
                <h2 className="text-sm font-bold">提示音文件</h2>
              </div>
              <div className="mt-3 flex min-w-0 items-center gap-2">
                <span className="truncate rounded-sm bg-muted px-2.5 py-1.5 text-xs font-medium text-muted-foreground" title={settings.soundPath ?? '项目 sounds/default-notification.wav'}>
                  {soundName}
                </span>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <Button variant="secondary" onClick={() => void chooseSound()} disabled={disabled}>
                <FolderOpen className="size-4" />
                选择音频
              </Button>
              <Button variant="secondary" onClick={resetSound} disabled={disabled || !settings.soundPath}>
                <RotateCcw className="size-4" />
                恢复默认
              </Button>
              <Button variant="secondary" onClick={() => void preview()} disabled={disabled || !settings.enabled}>
                <Play className={cn('size-4', busy === 'preview' && 'animate-pulse')} />
                试听
              </Button>
            </div>
          </div>

          <div className="flex min-h-[70px] items-center justify-between gap-5 border-t border-border bg-[#fafbfc] px-6 py-3.5">
            <div className="min-w-0">
              <p className="text-xs font-semibold text-foreground">配置范围：当前 Windows 用户的全部 Codex 项目</p>
              <p className="mt-1 truncate text-[11px] text-muted-foreground" title={scan?.codexHome}>
                {displayCodexPath(scan?.codexHome) || '等待发现 Codex 用户目录'}
              </p>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              {scan?.configured && (
                <Button variant="destructive" onClick={() => void remove()} disabled={disabled}>
                  <XCircle className="size-4" />
                  移除配置
                </Button>
              )}
              <Button onClick={() => void apply()} disabled={disabled || !scan?.codexFound}>
                <Save className="size-4" />
                应用到 Codex
              </Button>
            </div>
          </div>
        </section>

        <Dialog
          open={closeDialogOpen}
          onOpenChange={(open) => {
            if (busy !== 'close') setCloseDialogOpen(open)
          }}
        >
          <DialogContent>
            <div className="flex items-start justify-between gap-4">
              <div>
                <DialogTitle>关闭提示音管理器</DialogTitle>
                <DialogDescription className="mt-1.5">请选择退出程序，或让工具继续在系统托盘运行。</DialogDescription>
              </div>
              <Button
                variant="ghost"
                size="icon"
                className="-mr-2 -mt-2"
                onClick={() => void resolveCloseChoice('cancel')}
                disabled={busy === 'close'}
                aria-label="取消关闭"
              >
                <X className="size-4" aria-hidden="true" />
              </Button>
            </div>
            <div className="mt-6 flex items-center justify-end gap-2">
              <Button variant="ghost" onClick={() => void resolveCloseChoice('cancel')} disabled={busy === 'close'}>
                取消
              </Button>
              <Button variant="secondary" onClick={() => void resolveCloseChoice('tray')} disabled={busy === 'close'}>
                <Minimize2 className="size-4" aria-hidden="true" />
                最小化到托盘
              </Button>
              <Button variant="destructive" onClick={() => void resolveCloseChoice('exit')} disabled={busy === 'close'}>
                <LogOut className="size-4" aria-hidden="true" />
                退出程序
              </Button>
            </div>
          </DialogContent>
        </Dialog>

        <footer className="flex items-center justify-between px-1 pt-4 text-[11px] text-muted-foreground">
          <span>Codex Sound Manager</span>
          <div className="flex items-center gap-2">
            <Dialog>
              <DialogTrigger asChild>
                <Button variant="ghost" size="sm" className="h-7 px-2 text-[11px] font-medium">
                  <MessageCircle className="size-3.5" aria-hidden="true" />
                  联系开发者
                </Button>
              </DialogTrigger>
              <DialogContent>
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <DialogTitle>联系开发者</DialogTitle>
                    <DialogDescription className="mt-1">扫码加入交流群，反馈问题或交流使用经验</DialogDescription>
                  </div>
                  <DialogClose asChild>
                    <Button variant="ghost" size="icon" className="-mr-2 -mt-2" aria-label="关闭联系开发者窗口">
                      <X className="size-4" aria-hidden="true" />
                    </Button>
                  </DialogClose>
                </div>
                <img
                  src={communityQr}
                  alt="Codex 提示音管理器交流群二维码"
                  className="mx-auto mt-4 aspect-square w-[300px] max-w-full border border-border bg-white object-contain"
                />
              </DialogContent>
            </Dialog>
            <span>v{packageInfo.version}</span>
          </div>
        </footer>
      </main>

      {notice && (
        <div
          className={cn(
            'fixed bottom-5 left-1/2 z-50 flex max-w-[560px] -translate-x-1/2 items-center gap-2.5 rounded-md border bg-white px-4 py-3 text-sm font-semibold shadow-lg',
            notice.tone === 'success' && 'border-emerald-200 text-emerald-700',
            notice.tone === 'warning' && 'border-amber-200 text-amber-700',
            notice.tone === 'error' && 'border-red-200 text-red-700',
          )}
          role="status"
        >
          {notice.tone === 'success' ? <CheckCircle2 className="size-4" /> : notice.tone === 'warning' ? <AlertTriangle className="size-4" /> : <XCircle className="size-4" />}
          <span>{notice.message}</span>
        </div>
      )}
    </div>
  )
}
