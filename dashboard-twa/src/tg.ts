import WebApp from '@twa-dev/sdk';

WebApp.ready();
WebApp.expand();

export const tg = WebApp;

export function haptic(type: 'light' | 'medium' | 'heavy' | 'success' | 'error' | 'warning') {
  try {
    if (type === 'success' || type === 'error' || type === 'warning') {
      WebApp.HapticFeedback.notificationOccurred(type);
    } else {
      WebApp.HapticFeedback.impactOccurred(type);
    }
  } catch {}
}

export function showMainButton(text: string, color: string, onClick: () => void) {
  WebApp.MainButton.setParams({ text, color, text_color: '#fff' });
  WebApp.MainButton.show();
  WebApp.MainButton.onClick(onClick);
}

export function hideMainButton() {
  WebApp.MainButton.hide();
  (WebApp.MainButton as any).offClick();
}

export function showToast(message: string) {
  WebApp.showPopup({ title: 'FlowLink', message } as any);
}
