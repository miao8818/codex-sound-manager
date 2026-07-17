import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Code2, Volume2, VolumeX } from 'lucide-react'
import { createRoot } from 'react-dom/client'
import './floating.css'

type RuntimePreferences = {
  soundEnabled: boolean
  floatingBallEnabled: boolean
}

type PrimaryPress = {
  time: number
  screenX: number
  screenY: number
}

const DOUBLE_PRESS_INTERVAL_MS = 650
const DOUBLE_PRESS_DISTANCE_PX = 18
const floatingWindow = getCurrentWindow()

function FloatingBall() {
  const [enabled, setEnabled] = useState(true)
  const [busy, setBusy] = useState(true)
  const lastPrimaryPressRef = useRef<PrimaryPress | null>(null)

  useEffect(() => {
    let disposed = false
    const unlisteners: Array<() => void> = []

    void invoke<RuntimePreferences>('get_runtime_preferences')
      .then((preferences) => {
        if (!disposed) {
          setEnabled(preferences.soundEnabled)
        }
      })
      .catch(() => undefined)
      .finally(() => {
        if (!disposed) {
          setBusy(false)
        }
      })

    void listen<boolean>('sound-enabled-changed', (event) => {
      if (!disposed) {
        setEnabled(event.payload)
      }
    }).then((unlisten) => {
      if (disposed) {
        unlisten()
      } else {
        unlisteners.push(unlisten)
      }
    }).catch(() => undefined)

    return () => {
      disposed = true
      unlisteners.forEach((unlisten) => unlisten())
    }
  }, [])

  const toggleSound = async () => {
    if (busy) return
    setBusy(true)
    try {
      setEnabled(await invoke<boolean>('toggle_sound_enabled'))
    } catch {
      return
    } finally {
      setBusy(false)
    }
  }

  const showMainWindow = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault()
    void invoke('show_main_window').catch(() => undefined)
  }

  const handleMouseDown = (event: React.MouseEvent<HTMLButtonElement>) => {
    if (event.button !== 0) return

    event.preventDefault()
    const currentPress: PrimaryPress = {
      time: event.timeStamp,
      screenX: event.screenX,
      screenY: event.screenY,
    }
    const previousPress = lastPrimaryPressRef.current
    const isFallbackDoublePress = previousPress !== null
      && currentPress.time - previousPress.time <= DOUBLE_PRESS_INTERVAL_MS
      && Math.hypot(
        currentPress.screenX - previousPress.screenX,
        currentPress.screenY - previousPress.screenY,
      ) <= DOUBLE_PRESS_DISTANCE_PX
    const isDoublePress = event.detail === 2 || isFallbackDoublePress
    lastPrimaryPressRef.current = isDoublePress ? null : currentPress

    if (isDoublePress) {
      void toggleSound()
      return
    }

    window.setTimeout(() => {
      if (lastPrimaryPressRef.current === currentPress) {
        lastPrimaryPressRef.current = null
      }
    }, DOUBLE_PRESS_INTERVAL_MS)
    void floatingWindow.startDragging().catch(() => undefined)
  }

  const handleClick = (event: React.MouseEvent<HTMLButtonElement>) => {
    if (event.detail === 0) {
      void toggleSound()
    }
  }

  const label = enabled ? '提示音已开启，双击关闭' : '提示音已关闭，双击开启'
  const StatusIcon = enabled ? Volume2 : VolumeX

  return (
    <div className="floating-stage">
      <button
        className={`floating-orb ${enabled ? 'is-enabled' : 'is-disabled'}`}
        onContextMenu={showMainWindow}
        onMouseDown={handleMouseDown}
        onClick={handleClick}
        aria-label={label}
        aria-pressed={enabled}
        aria-disabled={busy}
        title={`${label}；按住任意位置拖动；右键打开主窗口`}
      >
        <span className="orb-shell" aria-hidden="true">
          <span className="orb-grid" />
          <span className="orb-glint" />
          <span className="tech-orbit">
            <span className="orbit-node orbit-node-a" />
            <span className="orbit-node orbit-node-b" />
          </span>
          <span className="circuit circuit-left" />
          <span className="circuit circuit-right" />
          <Code2 className="code-icon" />
          <span className="sound-status">
            <StatusIcon />
          </span>
        </span>
      </button>
    </div>
  )
}

createRoot(document.getElementById('floating-root')!).render(
  <FloatingBall />,
)
