import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import en from './en';
import ru from './ru';
i18n.use(initReactI18next).init({
    resources: { en: { translation: en }, ru: { translation: ru } },
    lng: localStorage.getItem('flowlink_lang') || (navigator.language.startsWith('ru') ? 'ru' : 'en'),
    fallbackLng: 'en',
    interpolation: { escapeValue: false },
});
export default i18n;
