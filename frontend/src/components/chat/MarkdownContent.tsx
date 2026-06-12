import type { CSSProperties } from 'react';
import ReactMarkdown, { type Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';

interface MarkdownContentProps {
  content: string;
  compact?: boolean;
}

function markdownStyles(compact: boolean): CSSProperties {
  return {
    fontSize: compact ? '11px' : '14px',
    lineHeight: compact ? 1.35 : 1.65,
    wordBreak: 'break-word',
    overflowWrap: 'break-word',
  };
}

function markdownComponents(compact: boolean): Components {
  const blockMargin = compact ? '2px 0' : '4px 0';

  return {
    code(props) {
      const { className, children } = props;
      const inline = !className;
      if (inline) {
        return (
          <code style={{
            background: 'var(--bg-secondary, rgba(0,0,0,0.06))',
            borderRadius: 3,
            padding: compact ? '0 3px' : '1px 5px',
            fontSize: '0.9em',
            fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace',
          }}>
            {children}
          </code>
        );
      }
      return (
        <pre style={{
          background: 'var(--bg-secondary, rgba(0,0,0,0.05))',
          borderRadius: compact ? 6 : 8,
          padding: compact ? '6px 7px' : '12px 14px',
          overflowX: 'auto',
          fontSize: compact ? '10px' : '13px',
          fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace',
          lineHeight: 1.5,
          margin: compact ? '3px 0' : '8px 0',
        }}>
          <code className={className}>{children}</code>
        </pre>
      );
    },

    p({ children }) { return <p style={{ margin: compact ? '0 0 3px' : '0 0 6px' }}>{children}</p>; },
    ul({ children }) { return <ul style={{ margin: blockMargin, paddingLeft: compact ? 14 : 20 }}>{children}</ul>; },
    ol({ children }) { return <ol style={{ margin: blockMargin, paddingLeft: compact ? 14 : 20 }}>{children}</ol>; },
    li({ children }) { return <li style={{ margin: compact ? '1px 0' : '2px 0' }}>{children}</li>; },
    blockquote({ children }) {
      return (
        <blockquote style={{
          borderLeft: '3px solid var(--accent, #0a73ff)',
          margin: compact ? '3px 0' : '6px 0',
          paddingLeft: compact ? 7 : 12,
          color: 'var(--text-secondary, #666)',
        }}>
          {children}
        </blockquote>
      );
    },
    table({ children }) {
      return (
        <div style={{ overflowX: 'auto', margin: compact ? '4px 0' : '8px 0' }}>
          <table style={{ borderCollapse: 'collapse', fontSize: compact ? '10px' : '13px', width: '100%' }}>{children}</table>
        </div>
      );
    },
    th({ children }) {
      return (
        <th style={{
          border: '1px solid var(--border, #ddd)',
          padding: compact ? '3px 5px' : '6px 10px',
          background: 'var(--bg-secondary, rgba(0,0,0,0.04))',
          textAlign: 'left',
          fontWeight: 600,
        }}>{children}</th>
      );
    },
    td({ children }) {
      return <td style={{ border: '1px solid var(--border, #ddd)', padding: compact ? '3px 5px' : '5px 10px' }}>{children}</td>;
    },
    hr() {
      return <hr style={{ border: 'none', borderTop: '1px solid var(--border, #ddd)', margin: compact ? '5px 0' : '10px 0' }} />;
    },
    a({ children, href }) {
      return (
        <a href={href} target="_blank" rel="noopener noreferrer"
          style={{ color: 'var(--accent, #0a73ff)', textDecoration: 'underline' }}>
          {children}
        </a>
      );
    },
    h1({ children }) { return <h1 style={{ fontSize: compact ? '1.08em' : '1.3em', fontWeight: 700, margin: compact ? '3px 0 2px' : '10px 0 4px' }}>{children}</h1>; },
    h2({ children }) { return <h2 style={{ fontSize: compact ? '1.04em' : '1.15em', fontWeight: 600, margin: compact ? '3px 0 2px' : '8px 0 4px' }}>{children}</h2>; },
    h3({ children }) { return <h3 style={{ fontSize: compact ? '1em' : '1.05em', fontWeight: 600, margin: compact ? '2px 0' : '6px 0 3px' }}>{children}</h3>; },
    strong({ children }) { return <strong style={{ fontWeight: 600 }}>{children}</strong>; },
    em({ children }) { return <em>{children}</em>; },
  };
}

export function MarkdownContent({ content, compact = false }: MarkdownContentProps) {
  return (
    <div style={markdownStyles(compact)}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents(compact)}>
        {content}
      </ReactMarkdown>
    </div>
  );
}
