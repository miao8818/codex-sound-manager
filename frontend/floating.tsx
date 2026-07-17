import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Code2, Volume2, VolumeX } from 'lucide-react'
import { createRoot } from 'react-dom/client'
import './floating.css'

type RuntimePreferences = {
  soundEnabled: boolean
  floatingBallEnabled: boolean
}

type FloatingPrimaryAction = {
  moved: boolean
  soundEnabled: boolean
}

type FloatingGesture = {
  pointerId: number
  startX: number
  startY: number
  deltaX: number
  deltaY: number
  moved: boolean
  finishing: boolean
  animationFrame: number | null
  commandChain: Promise<unknown>
}

const DRAG_THRESHOLD_PX = 3

function FloatingBall() {
  const [enabled, setEnabled] = useState(true)
  const [busy, setBusy] = useState(true)
  const [dragging, setDragging] = useState(false)
  const gestureRef = useRef<FloatingGesture | null>(null)

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

  const showMainWindow = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault()
    void invoke('show_main_window').catch(() => undefined)
  }

  const queuePositionUpdate = (gesture: FloatingGesture) => {
    if (gesture.animationFrame !== null) return
    gesture.animationFrame = requestAnimationFrame(() => {
      gesture.animationFrame = null
      const { deltaX, deltaY } = gesture
      gesture.commandChain = gesture.commandChain.then(() => invoke('update_floating_drag', {
        deltaX,
        deltaY,
      }))
    })
  }

  const handlePointerDown = (event: React.PointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0 || gestureRef.current) return
    event.preventDefault()
    event.currentTarget.setPointerCapture(event.pointerId)
    gestureRef.current = {
      pointerId: event.pointerId,
      startX: event.screenX,
      startY: event.screenY,
      deltaX: 0,
      deltaY: 0,
      moved: false,
      finishing: false,
      animationFrame: null,
      commandChain: invoke('begin_floating_drag'),
    }
    setDragging(true)
  }

  const handlePointerMove = (event: React.PointerEvent<HTMLButtonElement>) => {
    const gesture = gestureRef.current
    if (!gesture || gesture.pointerId !== event.pointerId || gesture.finishing) return
    gesture.deltaX = event.screenX - gesture.startX
    gesture.deltaY = event.screenY - gesture.startY
    gesture.moved ||= Math.hypot(gesture.deltaX, gesture.deltaY) >= DRAG_THRESHOLD_PX
    if (gesture.moved) {
      queuePositionUpdate(gesture)
    }
  }

  const finishGesture = async (
    event: React.PointerEvent<HTMLButtonElement>,
    cancelled: boolean,
  ) => {
    const gesture = gestureRef.current
    if (!gesture || gesture.pointerId !== event.pointerId || gesture.finishing) return
    event.preventDefault()
    gesture.finishing = true
    gesture.deltaX = event.screenX - gesture.startX
    gesture.deltaY = event.screenY - gesture.startY
    gesture.moved ||= Math.hypot(gesture.deltaX, gesture.deltaY) >= DRAG_THRESHOLD_PX
    if (gesture.animationFrame !== null) {
      cancelAnimationFrame(gesture.animationFrame)
      gesture.animationFrame = null
    }
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
    try {
      await gesture.commandChain
      if (gesture.moved) {
        await invoke('update_floating_drag', {
          deltaX: gesture.deltaX,
          deltaY: gesture.deltaY,
        })
      }
      const result = await invoke<FloatingPrimaryAction>('end_floating_drag', {
        shouldToggle: !cancelled && !gesture.moved && !busy,
      })
      setEnabled(result.soundEnabled)
    } catch {
      void invoke('end_floating_drag', { shouldToggle: false }).catch(() => undefined)
      return
    } finally {
      if (gestureRef.current === gesture) {
        gestureRef.current = null
      }
      setDragging(false)
    }
  }

  const label = enabled ? '提示音已开启，点击关闭' : '提示音已关闭，点击开启'
  const StatusIcon = enabled ? Volume2 : VolumeX

  return (
    <div className="floating-stage">
      <button
        className={`floating-orb ${enabled ? 'is-enabled' : 'is-disabled'} ${dragging ? 'is-dragging' : ''}`}
        onContextMenu={showMainWindow}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={(event) => void finishGesture(event, false)}
        onPointerCancel={(event) => void finishGesture(event, true)}
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
