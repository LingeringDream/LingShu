import { MarkdownContent } from '../chat/MarkdownContent';

interface PetDialogReplyProps {
  content: string;
}

export function PetDialogReply({ content }: PetDialogReplyProps) {
  return (
    <div
      role="region"
      aria-label="宠物回复"
      style={{
        minHeight: 36,
        maxHeight: 104,
        overflowY: 'auto',
        overflowX: 'hidden',
        color: '#40516f',
        marginBottom: 7,
        paddingRight: 2,
      }}
    >
      <MarkdownContent content={content} compact />
    </div>
  );
}
