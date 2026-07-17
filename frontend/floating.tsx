import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Code2, Volume2, VolumeX } from 'lucide-react'
import { createRoot } from 'react-dom/client'
import './floating.css'

type RuntimePreferences = {
  soundEnabled: boolean
  floatingBallEnabled: boolean
}

function FloatingBall() {
  const [enabled, setEnabled] = useState(true)
  const [busy, setBusy] = useState(true)

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

  const label = enabled ? '提示音已开启，点击关闭' : '提示音已关闭，点击开启'
  const StatusIcon = enabled ? Volume2 : VolumeX

  return (
    <div className="floating-stage">
      <button
        className={`floating-orb ${enabled ? 'is-enabled' : 'is-disabled'}`}
        data-tauri-drag-region="deep"
        onContextMenu={showMainWindow}
        onClick={() => void toggleSound()}
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
