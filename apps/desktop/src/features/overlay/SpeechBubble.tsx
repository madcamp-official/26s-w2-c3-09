import { useEffect, useRef, useState } from "react";

import { hideSpeechBubble, listenForSpeechBubbleText } from "./overlayApi";
import type { SpeechBubbleTailSide } from "./overlayApi";

const CHAR_INTERVAL_MS = 45;
const HOLD_AFTER_STREAM_MS = 3500;
const FADE_MS = 300;

/** Root component of the standalone speech-bubble window. Streams whatever line the mascot window
 * sends over one character at a time, holds it, fades out, then tells the backend to hide/close. */
export function SpeechBubble() {
  const [text, setText] = useState("");
  const [shownCount, setShownCount] = useState(0);
  const [tailSide, setTailSide] = useState<SpeechBubbleTailSide>("bottom");
  const [fading, setFading] = useState(false);
  const timersRef = useRef<number[]>([]);

  useEffect(() => {
    const clearTimers = () => {
      for (const id of timersRef.current) window.clearTimeout(id);
      timersRef.current = [];
    };

    const unlisten = listenForSpeechBubbleText((payload) => {
      clearTimers();
      setFading(false);
      setTailSide(payload.tail_side);
      setText(payload.text);
      setShownCount(0);

      const revealFrom = (index: number) => {
        setShownCount(index);
        if (index >= payload.text.length) {
          const holdTimer = window.setTimeout(() => {
            setFading(true);
            const fadeTimer = window.setTimeout(() => {
              void hideSpeechBubble().catch(() => undefined);
            }, FADE_MS);
            timersRef.current.push(fadeTimer);
          }, HOLD_AFTER_STREAM_MS);
          timersRef.current.push(holdTimer);
          return;
        }
        const timer = window.setTimeout(() => revealFrom(index + 1), CHAR_INTERVAL_MS);
        timersRef.current.push(timer);
      };
      revealFrom(0);
    });

    return () => {
      clearTimers();
      void unlisten.then((off) => off());
    };
  }, []);

  return (
    <div
      className={`speech-bubble-overlay speech-bubble-overlay--tail-${tailSide}${
        fading ? " is-fading" : ""
      }`}
    >
      <div className={`speech-bubble speech-bubble--tail-${tailSide}`}>
        <span className="speech-bubble-text">{text.slice(0, shownCount)}</span>
      </div>
    </div>
  );
}
