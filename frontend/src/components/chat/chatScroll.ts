type ChatScrollBehavior = 'auto' | 'smooth';

export function getChatScrollBehavior(isLoading: boolean): ChatScrollBehavior {
  return isLoading ? 'auto' : 'smooth';
}
