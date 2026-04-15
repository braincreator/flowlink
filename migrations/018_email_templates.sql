-- Email templates table for FlowLink
CREATE TABLE IF NOT EXISTS email_templates (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    subject VARCHAR(255) NOT NULL,
    body TEXT NOT NULL,
    status VARCHAR(20) DEFAULT 'active', -- active, inactive
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Insert basic email templates
INSERT INTO email_templates (name, subject, body) VALUES 
('welcome_email', 'Добро пожаловать в FlowLink!', '<!DOCTYPE html><html><head><meta charset="UTF-8"></head><body style="font-family: Arial, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px; line-height: 1.6;">
    <h1 style="color: #2563eb;">🎉 Добро пожаловать в FlowLink!</h1>
    <p>Мы рады видеть вас в платформе AI API Gateway для автоматизации бизнес-процессов.</p>
    <h2 style="color: #1f2937;">Что вы можете сделать:</h2>
    <ul style="color: #374151;">
        <li>🔗 Подключить Telegram ботов</li>
        <li>⚙️ Настраивать уведомления</li>
        <li>📊 Просматривать статистику</li>
        <li>💰 Управлять платежами</li>
    </ul>
    <p>Ваш тарифный план: <strong>Free</strong></p>
    <p>Максимальное количество серверов: <strong>1</strong></p>
    <hr style="margin: 20px 0; border: none; border-top: 1px solid #e5e7eb;">
    <p style="color: #6b7280; font-size: 14px;">С уважением,<br>Команда FlowLink</p>
</body></html>'),

('payment_success', 'Успешная оплата на FlowLink', '<!DOCTYPE html><html><head><meta charset="UTF-8"></head><body style="font-family: Arial, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px; line-height: 1.6;">
    <h1 style="color: #16a34a;">✅ Успешная оплата!</h1>
    <p>Ваш платеж успешно обработан.</p>
    <div style="background: #f3f4f6; padding: 15px; border-radius: 8px; margin: 20px 0;">
        <p><strong>Сумма:</strong> {{amount}} ₽</p>
        <p><strong>Тариф:</strong> {{plan}}</p>
        <p><strong>Период:</strong> {{period}}</p>
        <p><strong>ID транзакции:</strong> {{transaction_id}}</p>
    </div>
    <p>Ваш план обновлен, новые доступны немедленно.</p>
    <hr style="margin: 20px 0; border: none; border-top: 1px solid #e5e7eb;">
    <p style="color: #6b7280; font-size: 14px;">С уважением,<br>Команда FlowLink</p>
</body></html>'),

('password_reset', 'Восстановление пароля FlowLink', '<!DOCTYPE html><html><head><meta charset="UTF-8"></head><body style="font-family: Arial, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px; line-height: 1.6;">
    <h1 style="color: #dc2626;">🔐 Восстановление пароля</h1>
    <p>Вы запросили сброс пароля для аккаунта FlowLink. Если это не вы, проигнорируйте это письмо.</p>
    <div style="background: #f3f4f6; padding: 15px; border-radius: 8px; margin: 20px 0; text-align: center;">
        <p><strong>Код восстановления:</strong></p>
        <p style="font-size: 24px; font-weight: bold; color: #1f2937; margin: 10px 0;">{{code}}</p>
    </div>
    <p>Этот код действителен 15 минут.</p>
    <p>Вернитесь на страницу входа и введите этот код для сброса пароля.</p>
    <hr style="margin: 20px 0; border: none; border-top: 1px solid #e5e7eb;">
    <p style="color: #6b7280; font-size: 14px;">С уважением,<br>Команда FlowLink</p>
</body></html>'),

('trial_expiry_warning', 'Триал-период FlowLink истекает', '<!DOCTYPE html><html><head><meta charset="UTF-8"></head><body style="font-family: Arial, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px; line-height: 1.6;">
    <h1 style="color: #f59e0b;">⏰ Время триал-периода подходит к концу</h1>
    <p>Ваш бесплатный период на FlowLink заканчивается через 3 дня.</p>
    <div style="background: #fef3c7; padding: 15px; border-radius: 8px; margin: 20px 0; border-left: 4px solid #f59e0b;">
        <p><strong>Истекает:</strong> {{expiry_date}}</p>
        <p><strong>Текущий план:</strong> {{current_plan}}</p>
        <p><strong>Стоимость:</strong>{{plan_price}} ₽/месяц</p>
    </div>
    <p style="margin: 20px 0;"><a href="{{upgrade_url}}" style="background: #2563eb; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; display: inline-block;">Обновить план</a></p>
    <p>Продолжайте использовать FlowLink без перерывов!</p>
    <hr style="margin: 20px 0; border: none; border-top: 1px solid #e5e7eb;">
    <p style="color: #6b7280; font-size: 14px;">С уважением,<br>Команда FlowLink</p>
</body></html>'),

('email_verification', 'Подтвердите ваш email FlowLink', '<!DOCTYPE html><html><head><meta charset="UTF-8"></head><body style="font-family: Arial, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px; line-height: 1.6;">
    <h1 style="color: #2563eb;">✨ Подтвердите ваш email</h1>
    <p>Для завершения регистрации в FlowLink подтвердите ваш email адрес.</p>
    <div style="background: #f3f4f6; padding: 15px; border-radius: 8px; margin: 20px 0; text-align: center;">
        <p><strong>Код подтверждения:</strong></p>
        <p style="font-size: 24px; font-weight: bold; color: #1f2937; margin: 10px 0;">{{code}}</p>
    </div>
    <p>Вернитесь на страницу регистрации и введите этот код.</p>
    <hr style="margin: 20px 0; border: none; border-top: 1px solid #e5e7eb;">
    <p style="color: #6b7280; font-size: 14px;">С уважением,<br>Команда FlowLink</p>
</body></html>');

-- Add indexes for performance
CREATE INDEX IF NOT EXISTS idx_email_templates_name ON email_templates(name);
CREATE INDEX IF NOT EXISTS idx_email_templates_status ON email_templates(status);