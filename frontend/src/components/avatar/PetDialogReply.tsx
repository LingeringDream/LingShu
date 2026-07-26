import { useEffect, useRef, useState } from 'react';
import { MarkdownContent } from '../chat/MarkdownContent';
import { ArrowUpRightIcon } from '../icons';

interface PetDialogReplyProps {
  content: string;
  /** True while the reply is still streaming in. Controls scroll behaviour:
   *  follow the bottom during streaming, then snap back to the top once the
   *  reply settles so the user reads it from the beginning. */
  streaming?: boolean;
  /** Open the full reply in the main console. Shown only when the reply
   *  overflows the cramped pet dialog. Omit to hide the affordance. */
  onExpand?: () => void;
  maxHeight?: number;
  variant?: 'full' | 'compact';
}

export function PetDialogReply({
  content,
  streaming = false,
  onExpand,
  maxHeight = 150,
  variant = 'full',
}: PetDialogReplyProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [overflowing, setOverflowing] = useState(false);
  const compact = variant === 'compact';

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
    <div style={{
      marginBottom: compact ? 0 : 7,
      display: compact ? 'flex' : 'block',
      alignItems: compact ? 'flex-start' : undefined,
      gap: compact ? 4 : undefined,
      minWidth: 0,
    }}>
      <div
        ref={scrollRef}
        role="region"
        aria-label="宠物回复"
        style={{
          flex: compact ? 1 : undefined,
          minWidth: 0,
          minHeight: compact ? 20 : 36,
          maxHeight,
          overflowY: 'auto',
          overflowX: 'hidden',
          color: '#40516f',
          paddingRight: 2,
          fontSize: compact ? 11 : undefined,
          lineHeight: compact ? 1.3 : undefined,
        }}
      >
        <MarkdownContent content={content} compact />
      </div>
      {overflowing && onExpand && (
        <button
          type="button"
          aria-label="在主窗口查看完整回复"
          onClick={onExpand}
          style={{
            display: compact ? 'inline-flex' : 'block',
            alignItems: compact ? 'center' : undefined,
            justifyContent: compact ? 'center' : undefined,
            flex: compact ? '0 0 auto' : undefined,
            width: compact ? 32 : '100%',
            height: compact ? 22 : undefined,
            marginTop: compact ? 0 : 4,
            padding: compact ? 0 : '3px 0',
            border: 0,
            borderRadius: 6,
            background: 'rgba(46,107,255,0.08)',
            color: '#2e6bff',
            fontSize: 10,
            fontWeight: 600,
            cursor: 'pointer',
          }}
        >
          {compact ? '完整' : '在主窗口查看完整回复'}{' '}
          <ArrowUpRightIcon size={11} style={{ verticalAlign: '-1px' }} />
        </button>
      )}
    </div>
  );
}
