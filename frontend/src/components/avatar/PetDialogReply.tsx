import { useEffect, useRef, useState } from 'react';
import { MarkdownContent } from '../chat/MarkdownContent';

interface PetDialogReplyProps {
  content: string;
  /** True while the reply is still streaming in. Controls scroll behaviour:
   *  follow the bottom during streaming, then snap back to the top once the
   *  reply settles so the user reads it from the beginning. */
  streaming?: boolean;
  /** Open the full reply in the main console. Shown only when the reply
   *  overflows the cramped pet dialog. Omit to hide the affordance. */
  onExpand?: () => void;
}

export function PetDialogReply({ content, streaming = false, onExpand }: PetDialogReplyProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [overflowing, setOverflowing] = useState(false);

  // Surface the "view full reply" affordance whenever the content overflows
  // the cramped dialog. Scroll handling: while streaming, follow the bottom
  // only if the user is already near it (so reading earlier text mid-stream
  // isn't yanked away); once settled, snap to the top so long replies are read
  // from the start instead of being pinned to the end (the original bug).
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    setOverflowing(el.scrollHeight - el.clientHeight > 2);
    if (streaming) {
      const nearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
      if (nearBottom) el.scrollTop = el.scrollHeight;
    } else {
      el.scrollTop = 0;
    }
  }, [content, streaming]);

  return (
    <div style={{ marginBottom: 7 }}>
      <div
        ref={scrollRef}
        role="region"
        aria-label="宠物回复"
        style={{
          minHeight: 36,
          maxHeight: 150,
          overflowY: 'auto',
          overflowX: 'hidden',
          color: '#40516f',
          paddingRight: 2,
        }}
      >
        <MarkdownContent content={content} compact />
      </div>
      {overflowing && onExpand && (
        <button
          type="button"
          onClick={onExpand}
          style={{
            display: 'block',
            width: '100%',
            marginTop: 4,
            padding: '3px 0',
            border: 0,
            borderRadius: 6,
            background: 'rgba(46,107,255,0.08)',
            color: '#2e6bff',
            fontSize: 10,
            fontWeight: 600,
            cursor: 'pointer',
          }}
        >
          在主窗口查看完整回复 ↗
        </button>
      )}
    </div>
  );
}
