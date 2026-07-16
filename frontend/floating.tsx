import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Code2, GripHorizontal, Volume2, VolumeX } from 'lucide-react'
import { createRoot } from 'react-dom/client'
import './floating.css'

type RuntimePreferences = {
  soundEnabled: boolean
  floatingBallEnabled: boolean
}

function FloatingBall() {
  const [enabled, setEnabled] = useState(true)
  const [busy, setBusy] = useState(true)
  const dragPoint = useRef<{ pointerId: number; x: number; y: number } | null>(null)
  const dragQueue = useRef<Promise<unknown>>(Promise.resolve())

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

  const showMainWindow = (event: React.MouseEvent<HTMLDivElement>) => {
    event.preventDefault()
    void invoke('show_main_window').catch(() => undefined)
  }

  const beginDragging = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return
    event.preventDefault()
    event.currentTarget.setPointerCapture(event.pointerId)
    dragPoint.current = { pointerId: event.pointerId, x: event.screenX, y: event.screenY }
  }

  const continueDragging = (event: React.PointerEvent<HTMLDivElement>) => {
    const previous = dragPoint.current
    if (!previous || previous.pointerId !== event.pointerId) return
    const deltaX = event.screenX - previous.x
    const deltaY = event.screenY - previous.y
    if (deltaX === 0 && deltaY === 0) return
    dragPoint.current = { pointerId: event.pointerId, x: event.screenX, y: event.screenY }
    dragQueue.current = dragQueue.current
      .then(() => invoke('move_floating_ball_by', { deltaX, deltaY }))
      .catch(() => undefined)
  }

  const endDragging = (event: React.PointerEvent<HTMLDivElement>) => {
    if (dragPoint.current?.pointerId !== event.pointerId) return
    dragPoint.current = null
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
  }

  const label = enabled ? '提示音已开启，点击关闭' : '提示音已关闭，点击开启'
  const StatusIcon = enabled ? Volume2 : VolumeX

  return (
    <div className={`floating-shell ${enabled ? 'is-enabled' : 'is-disabled'}`} onContextMenu={showMainWindow}>
      <div
        className="drag-handle"
        title="按住拖动悬浮球"
        onPointerDown={beginDragging}
        onPointerMove={continueDragging}
        onPointerUp={endDragging}
        onPointerCancel={endDragging}
      >
        <GripHorizontal aria-hidden="true" />
      </div>
      <button className="sound-toggle" onClick={() => void toggleSound()} disabled={busy} aria-label={label} title={`${label}；右键打开主窗口`}>
        <Code2 className="code-icon" aria-hidden="true" />
        <span className="sound-status" aria-hidden="true">
          <StatusIcon />
        </span>
      </button>
    </div>
  )
}

createRoot(document.getElementById('floating-root')!).render(
  <FloatingBall />,
)
