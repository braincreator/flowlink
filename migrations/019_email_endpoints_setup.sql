-- Add email triggers for automated sending
CREATE OR REPLACE FUNCTION send_welcome_email_on_registration()
RETURNS TRIGGER AS $$
BEGIN
    -- Send welcome email to new users
    PERFORM pg_notify(
        'email_service',
        jsonb_build_object(
            'action', 'send_welcome',
            'email', NEW.email,
            'name', COALESCE(NEW.name, 'User'),
            'user_id', NEW.account_id
        )::text
    );
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_send_welcome_email_after_registration
AFTER INSERT ON accounts
FOR EACH ROW
EXECUTE FUNCTION send_welcome_email_on_registration();

-- Add email queue table for bulk operations
CREATE TABLE IF NOT EXISTS email_queue (
    id SERIAL PRIMARY KEY,
    email VARCHAR(255) NOT NULL,
    template_name VARCHAR(100) NOT NULL,
    template_data JSONB,
    status VARCHAR(20) DEFAULT 'pending', -- pending, sent, failed
    attempts INTEGER DEFAULT 0,
    max_attempts INTEGER DEFAULT 3,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    sent_at TIMESTAMP WITH TIME ZONE,
    error_message TEXT
);

-- Add index for email queue processing
CREATE INDEX IF NOT EXISTS idx_email_queue_status ON email_queue(status);
CREATE INDEX IF NOT EXISTS idx_email_queue_created_at ON email_queue(created_at);

-- Function to add email to queue
CREATE OR REPLACE FUNCTION add_email_to_queue(
    p_email VARCHAR(255),
    p_template_name VARCHAR(100),
    p_template_data JSONB DEFAULT '{}'::jsonb
)
RETURNS INTEGER AS $$
DECLARE
    v_queue_id INTEGER;
BEGIN
    INSERT INTO email_queue (email, template_name, template_data, status)
    VALUES (p_email, p_template_name, p_template_data, 'pending')
    RETURNING id INTO v_queue_id;
    
    -- Notify email service worker
    PERFORM pg_notify('email_worker', jsonb_build_object('action', 'process_queue', 'queue_id', v_queue_id)::text);
    
    RETURN v_queue_id;
END;
$$ LANGUAGE plpgsql;

-- Function to process email queue (for worker)
CREATE OR REPLACE FUNCTION process_email_queue(p_queue_id INTEGER)
RETURNS BOOLEAN AS $$
DECLARE
    v_email VARCHAR(255);
    v_template_name VARCHAR(100);
    v_template_data JSONB;
    v_attempts INTEGER;
    v_max_attempts INTEGER;
    v_success BOOLEAN := false;
BEGIN
    -- Get email details
    SELECT email, template_name, template_data, attempts, max_attempts
    INTO v_email, v_template_name, v_template_data, v_attempts, v_max_attempts
    FROM email_queue
    WHERE id = p_queue_id AND status = 'pending';
    
    IF NOT FOUND THEN
        RETURN false;
    END IF;
    
    -- Try to send email
    BEGIN
        -- Here you would call the email sending function
        -- For now, simulate sending success
        v_success := true;
        
        UPDATE email_queue
        SET status = 'sent',
            sent_at = NOW(),
            attempts = v_attempts + 1
        WHERE id = p_queue_id;
        
    EXCEPTION WHEN OTHERS THEN
        -- Mark as failed
        v_attempts := v_attempts + 1;
        
        UPDATE email_queue
        SET status = CASE WHEN v_attempts >= v_max_attempts THEN 'failed' ELSE 'pending' END,
            attempts = v_attempts,
            error_message = SQLERRM,
            sent_at = CASE WHEN v_attempts >= v_max_attempts THEN NOW() ELSE NULL END
        WHERE id = p_queue_id;
        
        -- Retry logic
        IF v_attempts < v_max_attempts AND status = 'pending' THEN
            PERFORM pg_notify('email_worker', jsonb_build_object('action', 'process_queue', 'queue_id', p_queue_id)::text);
        END IF;
        
        v_success := false;
    END;
    
    RETURN v_success;
END;
$$ LANGUAGE plpgsql;