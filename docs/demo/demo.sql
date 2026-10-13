SELECT region, COUNT(*) AS rows, AVG(amount) AS average_amount
FROM data
WHERE active = true
GROUP BY region
ORDER BY region;
